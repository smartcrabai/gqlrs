use std::collections::HashSet;

use darling::FromMeta;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Error, Expr, ExprLit, ExprPath, FnArg, GenericArgument, Ident, ImplItemFn, Lifetime,
    Lit, LitStr, Meta, Pat, PatIdent, PathArguments, Type, TypeGroup, TypeParamBound, TypeParen,
    TypeReference, parse_quote,
    visit::Visit,
    visit_mut::{self, VisitMut},
};
use thiserror::Error;

use crate::args::{self, Deprecation, TypeDirectiveLocation, Visible};

#[derive(Error, Debug)]
pub enum GeneratorError {
    #[error("{0}")]
    Syn(#[from] syn::Error),

    #[error("{0}")]
    Darling(#[from] darling::Error),
}

impl GeneratorError {
    pub fn write_errors(self) -> TokenStream {
        match self {
            GeneratorError::Syn(err) => err.to_compile_error(),
            GeneratorError::Darling(err) => err.write_errors(),
        }
    }
}

pub type GeneratorResult<T> = std::result::Result<T, GeneratorError>;

pub fn get_crate_path(crate_path: &Option<syn::Path>, internal: bool) -> syn::Path {
    if internal {
        parse_quote! { crate }
    } else if let Some(path) = crate_path {
        path.clone()
    } else {
        let name = match crate_name("gqlrs").or_else(|_| crate_name("async-graphql")) {
            Ok(FoundCrate::Name(name)) => name,
            Ok(FoundCrate::Itself) | Err(_) => "gqlrs".to_string(),
        };
        let ident = Ident::new(&name, Span::call_site());
        parse_quote! { ::#ident }
    }
}

pub fn generate_guards(
    crate_name: &syn::Path,
    expr: &Expr,
    map_err: TokenStream,
    on_error: TokenStream,
) -> GeneratorResult<TokenStream> {
    let code = quote! {{
        use #crate_name::GuardExt;
        #expr
    }};
    Ok(quote! {
        if let ::std::result::Result::Err(err) = #crate_name::Guard::check(&#code, &ctx).await #map_err {
            #on_error
        }
    })
}

pub fn nullable_type_check(crate_name: &syn::Path, ty: &Type) -> TokenStream {
    if is_output_type_nullable(ty) {
        quote!(true)
    } else {
        quote!(!<#ty as #crate_name::OutputTypeMarker>::qualified_type_name().ends_with('!'))
    }
}

pub fn output_type_create_type_info(crate_name: &syn::Path, ty: &Type) -> TokenStream {
    quote! {
        <#ty as #crate_name::OutputTypeMarker>::create_type_info(registry)
    }
}

pub fn nullable_output_type_create_type_info(crate_name: &syn::Path, ty: &Type) -> TokenStream {
    let create_type_info = output_type_create_type_info(crate_name, ty);
    quote! {{
        let ty = #create_type_info;
        if let ::std::option::Option::Some(ty) = ty.strip_suffix('!') {
            ::std::string::ToString::to_string(ty)
        } else {
            ty
        }
    }}
}

/// Check if a field should be treated as nullable, considering both the type
/// and the nullable attribute. Returns a TokenStream that evaluates to a
/// boolean at runtime.
pub fn nullable_field_check(crate_name: &syn::Path, ty: &Type, nullable_attr: bool) -> TokenStream {
    if nullable_attr {
        quote!(true)
    } else {
        nullable_type_check(crate_name, ty)
    }
}

pub fn create_output_type_info(
    crate_name: &syn::Path,
    ty: &Type,
    nullable_attr: bool,
) -> TokenStream {
    if nullable_attr {
        quote!(<::std::option::Option<#ty> as #crate_name::OutputTypeMarker>::create_type_info(registry))
    } else {
        quote!(<#ty as #crate_name::OutputTypeMarker>::create_type_info(registry))
    }
}
fn is_output_type_nullable(ty: &Type) -> bool {
    match ty {
        Type::Group(ty) => is_output_type_nullable(&ty.elem),
        Type::Paren(ty) => is_output_type_nullable(&ty.elem),
        Type::Reference(TypeReference { elem, .. }) => is_output_type_nullable(elem),
        Type::Path(ty) => {
            let Some(segment) = ty.path.segments.last() else {
                return false;
            };
            let ident = segment.ident.to_string();
            if ident == "Option" || ident == "Weak" {
                return true;
            }
            // Result<T, E> is always nullable: errors become null + error in response
            if ident == "Result" || ident == "FieldResult" {
                return true;
            }
            if matches!(ident.as_str(), "Arc" | "Box" | "Cow") {
                return first_generic_type(&segment.arguments).is_some_and(is_output_type_nullable);
            }
            false
        }
        _ => false,
    }
}

/// Strip transparent wrappers (`Type::Group`, `Type::Paren`) that macros may
/// emit. This is needed because declarative macros can wrap types in invisible
/// groups that would otherwise prevent proper detection of `&Context<'_>`
/// patterns.
pub fn unwrap_type(ty: &Type) -> &Type {
    match ty {
        Type::Group(TypeGroup { elem, .. }) => unwrap_type(elem),
        Type::Paren(TypeParen { elem, .. }) => unwrap_type(elem),
        _ => ty,
    }
}

fn first_generic_type(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    arguments.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

pub fn get_rustdoc(attrs: &[Attribute]) -> GeneratorResult<Option<TokenStream>> {
    let mut full_docs: Vec<TokenStream> = vec![];
    let mut combined_docs_literal = String::new();
    for attr in attrs {
        if let Meta::NameValue(nv) = &attr.meta
            && nv.path.is_ident("doc")
        {
            match &nv.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(doc), ..
                }) => {
                    let doc = doc.value();
                    let doc_str = doc.trim();
                    combined_docs_literal += "\n";
                    combined_docs_literal += doc_str;
                }
                Expr::Macro(include_macro) => {
                    if !combined_docs_literal.is_empty() {
                        combined_docs_literal += "\n";
                        let lit = LitStr::new(&combined_docs_literal, Span::call_site());
                        full_docs.push(quote!( #lit ));
                        combined_docs_literal.clear();
                    }
                    full_docs.push(quote!( #include_macro ));
                }
                _ => (),
            }
        }
    }

    if !combined_docs_literal.is_empty() {
        let lit = LitStr::new(&combined_docs_literal, Span::call_site());
        full_docs.push(quote!( #lit ));
        combined_docs_literal.clear();
    }

    Ok(if full_docs.is_empty() {
        None
    } else {
        Some(quote!(::core::primitive::str::trim(
            ::std::concat!( #( #full_docs ),* )
        )))
    })
}

fn generate_default_value(lit: &Lit) -> GeneratorResult<TokenStream> {
    match lit {
        Lit::Str(value) =>{
            let value = value.value();
            Ok(quote!({ ::std::borrow::ToOwned::to_owned(#value) }))
        }
        Lit::Int(value) => {
            let value = value.base10_parse::<i32>()?;
            Ok(quote!({ ::std::convert::TryInto::try_into(#value).unwrap_or_default() }))
        }
        Lit::Float(value) => {
            let value = value.base10_parse::<f64>()?;
            Ok(quote!({ ::std::convert::TryInto::try_into(#value).unwrap_or_default() }))
        }
        Lit::Bool(value) => {
            let value = value.value;
            Ok(quote!({ #value }))
        }
        _ => Err(Error::new_spanned(
            lit,
            "The default value type only be string, integer, float and boolean, other types should use default_with",
        ).into()),
    }
}

fn generate_default_with(lit: &LitStr) -> GeneratorResult<TokenStream> {
    let str = lit.value();
    let tokens: TokenStream = str
        .parse()
        .map_err(|err| GeneratorError::Syn(syn::Error::from(err)))?;
    Ok(quote! { (#tokens) })
}

pub fn generate_default(
    default: &Option<args::DefaultValue>,
    default_with: &Option<LitStr>,
) -> GeneratorResult<Option<TokenStream>> {
    match (default, default_with) {
        (Some(args::DefaultValue::Default), _) => {
            Ok(Some(quote! { ::std::default::Default::default() }))
        }
        (Some(args::DefaultValue::Value(lit)), _) => Ok(Some(generate_default_value(lit)?)),
        (None, Some(lit)) => Ok(Some(generate_default_with(lit)?)),
        (None, None) => Ok(None),
    }
}

pub fn get_cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().segments.is_empty() && attr.path().segments[0].ident == "cfg")
        .cloned()
        .collect()
}

pub fn parse_graphql_attrs<T: FromMeta + Default>(
    attrs: &[Attribute],
) -> GeneratorResult<Option<T>> {
    for attr in attrs {
        if attr.path().is_ident("graphql") {
            return Ok(Some(T::from_meta(&attr.meta)?));
        }
    }
    Ok(None)
}

pub fn remove_graphql_attrs(attrs: &mut Vec<Attribute>) {
    if let Some((idx, _)) = attrs
        .iter()
        .enumerate()
        .find(|(_, a)| a.path().is_ident("graphql"))
    {
        attrs.remove(idx);
    }
}

pub fn get_type_path_and_name(ty: &Type) -> GeneratorResult<(&Type, String)> {
    match ty {
        Type::Path(path) => Ok((
            ty,
            path.path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap(),
        )),
        Type::Group(TypeGroup { elem, .. }) => get_type_path_and_name(elem),
        Type::TraitObject(trait_object) => Ok((
            ty,
            trait_object
                .bounds
                .iter()
                .find_map(|bound| match bound {
                    TypeParamBound::Trait(t) => {
                        Some(t.path.segments.last().map(|s| s.ident.to_string()).unwrap())
                    }
                    _ => None,
                })
                .unwrap(),
        )),
        _ => Err(Error::new_spanned(ty, "Invalid type").into()),
    }
}

pub fn visible_fn(visible: &Option<Visible>) -> TokenStream {
    match visible {
        None | Some(Visible::None) => quote! { ::std::option::Option::None },
        Some(Visible::HiddenAlways) => quote! { ::std::option::Option::Some(|_| false) },
        Some(Visible::FnName(name)) => {
            quote! { ::std::option::Option::Some(#name) }
        }
    }
}

pub fn parse_complexity_expr(
    expr: Expr,
    arg_names: &HashSet<String>,
) -> GeneratorResult<(HashSet<String>, Expr)> {
    struct VisitComplexityExpr<'a> {
        variables: HashSet<String>,
        arg_names: &'a HashSet<String>,
    }

    impl<'a, 'b> Visit<'a> for VisitComplexityExpr<'b> {
        fn visit_expr_path(&mut self, i: &'a ExprPath) {
            if let Some(ident) = i.path.get_ident() {
                let name = ident.to_string();
                if name != "child_complexity" && self.arg_names.contains(&name) {
                    self.variables.insert(name);
                }
            }
        }
    }

    let mut visit = VisitComplexityExpr {
        arg_names,
        variables: HashSet::new(),
    };
    visit.visit_expr(&expr);
    Ok((visit.variables, expr))
}

pub fn gen_deprecation(deprecation: &Deprecation, crate_name: &syn::Path) -> TokenStream {
    match deprecation {
        Deprecation::NoDeprecated => {
            quote! { #crate_name::registry::Deprecation::NoDeprecated }
        }
        Deprecation::Deprecated {
            reason: Some(reason),
        } => {
            quote! { #crate_name::registry::Deprecation::Deprecated { reason: ::std::option::Option::Some(::std::string::ToString::to_string(#reason)) } }
        }
        Deprecation::Deprecated { reason: None } => {
            quote! { #crate_name::registry::Deprecation::Deprecated { reason: ::std::option::Option::None } }
        }
    }
}

pub fn extract_input_args<T: FromMeta + Default>(
    crate_name: &syn::Path,
    method: &mut ImplItemFn,
) -> GeneratorResult<Vec<(PatIdent, Type, T)>> {
    let mut args = Vec::new();
    let mut create_ctx = true;

    if method.sig.inputs.is_empty() {
        return Err(Error::new_spanned(
            &method.sig,
            "The self receiver must be the first parameter.",
        )
        .into());
    }

    for (idx, arg) in method.sig.inputs.iter_mut().enumerate() {
        if let FnArg::Receiver(receiver) = arg {
            if idx != 0 {
                return Err(Error::new_spanned(
                    receiver,
                    "The self receiver must be the first parameter.",
                )
                .into());
            }
        } else if let FnArg::Typed(pat) = arg {
            if idx == 0 {
                return Err(Error::new_spanned(
                    pat,
                    "The self receiver must be the first parameter.",
                )
                .into());
            }

            match (&*pat.pat, unwrap_type(&pat.ty)) {
                (Pat::Ident(arg_ident), Type::Reference(TypeReference { elem, .. })) => {
                    if let Type::Path(path) = elem.as_ref() {
                        if idx != 1 || path.path.segments.last().unwrap().ident != "Context" {
                            args.push((
                                arg_ident.clone(),
                                pat.ty.as_ref().clone(),
                                parse_graphql_attrs::<T>(&pat.attrs)?.unwrap_or_default(),
                            ));
                        } else {
                            create_ctx = false;
                        }
                    }
                }
                (Pat::Ident(arg_ident), ty) => {
                    args.push((
                        arg_ident.clone(),
                        ty.clone(),
                        parse_graphql_attrs::<T>(&pat.attrs)?.unwrap_or_default(),
                    ));
                    remove_graphql_attrs(&mut pat.attrs);
                }
                _ => {
                    return Err(Error::new_spanned(arg, "Invalid argument type.").into());
                }
            }
        }
    }

    if create_ctx {
        let arg = syn::parse2::<FnArg>(quote! { _: &#crate_name::Context<'_> }).unwrap();
        method.sig.inputs.insert(1, arg);
    }

    Ok(args)
}

pub struct RemoveLifetime;

impl VisitMut for RemoveLifetime {
    fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
        i.ident = Ident::new("_", Span::call_site());
        visit_mut::visit_lifetime_mut(self, i);
    }
}

pub fn gen_directive_calls(
    crate_name: &syn::Path,
    directive_calls: &[Expr],
    location: TypeDirectiveLocation,
) -> Vec<TokenStream> {
    directive_calls
        .iter()
        .map(|directive| {
            let directive_path = extract_directive_call_path(directive).expect(
                "Directive invocation expression format must be [<directive_path>::]<directive_name>::apply(<args>)",
            );
            let identifier = location.location_trait_identifier();
            quote!({
                <#directive_path as #crate_name::registry::location_traits::#identifier>::check();
                <#directive_path as #crate_name::TypeDirective>::register(&#directive_path, registry);
                #directive
            })
        })
        .collect::<Vec<_>>()
}

fn extract_directive_call_path(directive: &Expr) -> Option<syn::Path> {
    if let Expr::Call(expr) = directive
        && let Expr::Path(ref expr) = *expr.func
    {
        let mut path = expr.path.clone();
        if path.segments.pop()?.value().ident != "apply" {
            return None;
        }

        path.segments.pop_punct()?;

        return Some(path);
    }

    None
}

pub fn gen_boxed_trait(crate_name: &syn::Path) -> TokenStream {
    if cfg!(feature = "boxed-trait") {
        quote! {
            #[#crate_name::async_trait::async_trait]
        }
    } else {
        quote! {}
    }
}
