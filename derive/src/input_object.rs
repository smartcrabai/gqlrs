use darling::ast::Data;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Error, Expr, Type, ext::IdentExt};

use crate::{
    args::{self, RenameRuleExt, RenameTarget, TypeDirectiveLocation},
    utils::{
        GeneratorResult, gen_deprecation, gen_directive_calls, generate_default, get_crate_path,
        get_rustdoc, visible_fn,
    },
};

fn inferred_input_type(
    crate_name: &syn::Path,
    ty: &Type,
    input_using: Option<&Expr>,
) -> TokenStream2 {
    match input_using {
        Some(input_using) => quote! {{
            fn __gqlrs_input_type<T, F>(
                _converter: F,
                registry: &mut #crate_name::registry::Registry,
            ) -> ::std::string::String
            where
                T: #crate_name::InputType,
                F: ::std::ops::Fn(T) -> #ty,
            {
                <T as #crate_name::InputType>::create_type_info(registry)
            }

            __gqlrs_input_type(#input_using, registry)
        }},
        None => quote! { <#ty as #crate_name::InputType>::create_type_info(registry) },
    }
}

fn parse_input_value(
    crate_name: &syn::Path,
    ty: &Type,
    input_using: Option<&Expr>,
    value: TokenStream2,
) -> TokenStream2 {
    match input_using {
        Some(input_using) => quote! {{
            fn __gqlrs_parse_input<T, F>(
                converter: F,
                value: ::std::option::Option<#crate_name::Value>,
            ) -> ::std::result::Result<#ty, #crate_name::InputValueError<T>>
            where
                T: #crate_name::InputType,
                F: ::std::ops::Fn(T) -> #ty,
            {
                let value = <T as #crate_name::InputType>::parse(value)?;
                ::std::result::Result::Ok(converter(value))
            }

            __gqlrs_parse_input(#input_using, #value)
                .map_err(#crate_name::InputValueError::propagate)?
        }},
        None => quote! {
            #crate_name::InputType::parse(#value)
                .map_err(#crate_name::InputValueError::propagate)?
        },
    }
}

fn flatten_schema_fields(
    crate_name: &syn::Path,
    ty: &Type,
    input_using: Option<&Expr>,
    assert_generics: Option<TokenStream2>,
) -> TokenStream2 {
    match input_using {
        Some(input_using) => quote! {
            {
                fn __gqlrs_extend_input_fields<T, F>(
                    _converter: F,
                    registry: &mut #crate_name::registry::Registry,
                    fields: &mut #crate_name::indexmap::IndexMap<
                        ::std::string::String,
                        #crate_name::registry::MetaInputValue,
                    >,
                )
                where
                    T: #crate_name::InputObjectType,
                    F: ::std::ops::Fn(T) -> #ty,
                {
                    <T as #crate_name::InputType>::create_type_info(registry);
                    if let #crate_name::registry::MetaType::InputObject { input_fields, .. } =
                        registry.create_fake_input_type::<T>()
                    {
                        fields.extend(input_fields);
                    }
                }

                __gqlrs_extend_input_fields(#input_using, registry, &mut fields);
            }
        },
        None => quote! {
            #crate_name::static_assertions_next::assert_impl!(#assert_generics #ty: #crate_name::InputObjectType);
            <#ty as #crate_name::InputType>::create_type_info(registry);
            if let #crate_name::registry::MetaType::InputObject { input_fields, .. } =
                registry.create_fake_input_type::<#ty>()
            {
                fields.extend(input_fields);
            }
        },
    }
}

fn federation_field(
    crate_name: &syn::Path,
    ty: &Type,
    input_using: Option<&Expr>,
    name: &str,
) -> TokenStream2 {
    match input_using {
        Some(input_using) => quote! {
            {
                fn __gqlrs_push_federation_field<T, F>(
                    _converter: F,
                    res: &mut ::std::vec::Vec<::std::string::String>,
                    name: &::std::primitive::str,
                )
                where
                    T: #crate_name::InputType,
                    F: ::std::ops::Fn(T) -> #ty,
                {
                    if let ::std::option::Option::Some(fields) = <T as #crate_name::InputType>::federation_fields() {
                        res.push(::std::format!("{} {}", name, fields));
                    } else {
                        res.push(::std::string::ToString::to_string(name));
                    }
                }

                __gqlrs_push_federation_field(#input_using, &mut res, #name);
            }
        },
        None => quote! {
            if let ::std::option::Option::Some(fields) = <#ty as #crate_name::InputType>::federation_fields() {
                res.push(::std::format!("{} {}", #name, fields));
            } else {
                res.push(::std::string::ToString::to_string(#name));
            }
        },
    }
}

fn input_object_to_value(
    crate_name: &syn::Path,
    ty: &Type,
    input_using: Option<&Expr>,
    output_using: Option<&Expr>,
    value: TokenStream2,
) -> TokenStream2 {
    match (input_using, output_using) {
        (Some(input_using), Some(output_using)) => quote! {{
            fn __gqlrs_input_to_value<T, F, G>(
                _input_converter: F,
                output_converter: G,
                value: &#ty,
            ) -> #crate_name::Value
            where
                T: #crate_name::InputType,
                F: ::std::ops::Fn(T) -> #ty,
                G: ::std::ops::Fn(&#ty) -> T,
            {
                let value = output_converter(value);
                #crate_name::InputType::to_value(&value)
            }

            __gqlrs_input_to_value(#input_using, #output_using, #value)
        }},
        _ => quote! { <#ty as #crate_name::InputType>::to_value(#value) },
    }
}

pub fn generate(object_args: &args::InputObject) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let ident = &object_args.ident;
    let tags = object_args
        .tags
        .iter()
        .map(|tag| quote!(::std::string::ToString::to_string(#tag)))
        .collect::<Vec<_>>();
    let directives = gen_directive_calls(
        &crate_name,
        &object_args.directives,
        TypeDirectiveLocation::InputObject,
    );
    let s = match &object_args.data {
        Data::Struct(s) => s,
        _ => {
            return Err(
                Error::new_spanned(ident, "InputObject can only be applied to an struct.").into(),
            );
        }
    };

    for field in &s.fields {
        if field.ident.is_none() {
            return Err(Error::new_spanned(ident, "All fields must be named.").into());
        }
    }

    let gql_typename = if !object_args.name_type {
        let name = object_args
            .input_name
            .clone()
            .or_else(|| object_args.name.clone())
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));

        quote!(::std::borrow::Cow::Borrowed(#name))
    } else {
        quote!(<Self as #crate_name::TypeName>::type_name())
    };
    let gql_typename_string = if !object_args.name_type {
        let name = object_args
            .input_name
            .clone()
            .or_else(|| object_args.name.clone())
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote!(::std::string::ToString::to_string(#name))
    } else {
        quote!(::std::string::ToString::to_string(&#gql_typename))
    };

    let desc = get_rustdoc(&object_args.attrs)?;
    let has_desc = desc.is_some();
    let desc = desc
        .map(|s| quote! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});

    let mut get_fields = Vec::new();
    let mut put_fields = Vec::new();
    let mut fields = Vec::new();
    let mut schema_fields = Vec::new();
    let mut flatten_fields = Vec::new();
    let mut federation_fields = Vec::new();

    for field in &s.fields {
        let ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let name = field.name.clone().unwrap_or_else(|| {
            object_args
                .rename_fields
                .rename(ident.unraw().to_string(), RenameTarget::Field)
        });
        let inaccessible = field.inaccessible;
        let tags = field
            .tags
            .iter()
            .map(|tag| quote!(::std::string::ToString::to_string(#tag)))
            .collect::<Vec<_>>();

        let directive_invocations = gen_directive_calls(
            &crate_name,
            &field.directives,
            TypeDirectiveLocation::InputFieldDefinition,
        );

        if field.skip || field.skip_input {
            get_fields.push(quote! {
                let #ident: #ty = ::std::default::Default::default();
            });
            fields.push(ident);
            continue;
        }

        federation_fields.push(federation_field(
            &crate_name,
            ty,
            field.input_using.as_ref(),
            &name,
        ));

        let process_with = match field.process_with.as_ref() {
            Some(fn_path) => quote! { #fn_path(&mut #ident); },
            None => Default::default(),
        };

        let validators = field
            .validator
            .clone()
            .unwrap_or_default()
            .create_validators(
                &crate_name,
                quote!(&#ident),
                Some(quote!(.map_err(#crate_name::InputValueError::propagate))),
            )?;

        if field.flatten {
            flatten_fields.push((ident, ty));

            let assert_generics = (!object_args.generics.params.is_empty()).then(|| {
                let generics_list = &object_args.generics.params;
                quote! { for(#generics_list) }
            });

            let schema_fields_for_flatten =
                flatten_schema_fields(&crate_name, ty, field.input_using.as_ref(), assert_generics);
            schema_fields.push(quote! { #schema_fields_for_flatten });

            let parse_value = parse_input_value(
                &crate_name,
                ty,
                field.input_using.as_ref(),
                quote! {
                    ::std::option::Option::Some(#crate_name::Value::Object(::std::clone::Clone::clone(&obj)))
                },
            );
            get_fields.push(quote! {
                #[allow(unused_mut)]
                let mut #ident: #ty = #parse_value;
                #process_with
                #validators
            });

            fields.push(ident);

            let to_value = input_object_to_value(
                &crate_name,
                ty,
                field.input_using.as_ref(),
                field.output_using.as_ref(),
                quote! { &self.#ident },
            );
            put_fields.push(quote! {
                if let #crate_name::Value::Object(values) = #to_value {
                    map.extend(values);
                }
            });
            continue;
        }

        let desc = get_rustdoc(&field.attrs)?;
        let has_desc = desc.is_some();
        let default = generate_default(&field.default, &field.default_with)?;
        let schema_default = default.as_ref().map(|value| {
            let to_value = input_object_to_value(
                &crate_name,
                ty,
                field.input_using.as_ref(),
                field.output_using.as_ref(),
                quote! { &#value },
            );
            quote! {
                ::std::option::Option::Some(::std::string::ToString::to_string(&#to_value))
            }
        });
        let secret = field.secret;

        if let Some(default) = default {
            let parse_value = parse_input_value(
                &crate_name,
                ty,
                field.input_using.as_ref(),
                quote! { ::std::option::Option::Some(::std::clone::Clone::clone(&value)) },
            );
            get_fields.push(quote! {
                #[allow(non_snake_case)]
                let #ident: #ty = {
                    match obj.get(#name) {
                        ::std::option::Option::Some(value) => {
                            #[allow(unused_mut)]
                            let mut #ident: #ty = #parse_value;
                            #process_with
                            #ident

                        },
                        ::std::option::Option::None => #default,
                    }
                };
                #validators
            });
        } else {
            let parse_value = parse_input_value(
                &crate_name,
                ty,
                field.input_using.as_ref(),
                quote! { obj.get(#name).cloned() },
            );
            get_fields.push(quote! {
                #[allow(non_snake_case, unused_mut)]
                let mut #ident: #ty = #parse_value;
                #process_with
                #validators
            });
        }

        let to_value = input_object_to_value(
            &crate_name,
            ty,
            field.input_using.as_ref(),
            field.output_using.as_ref(),
            quote! { &self.#ident },
        );
        put_fields.push(quote! {
            map.insert(
                #crate_name::Name::new(#name),
                #to_value
            );
        });

        fields.push(ident);

        let has_visible = field.visible.is_some();
        let visible = visible_fn(&field.visible);
        let has_deprecation = !matches!(field.deprecation, args::Deprecation::NoDeprecated);
        let deprecation_expr = gen_deprecation(&field.deprecation, &crate_name);
        let has_tags = !tags.is_empty();
        let has_directive_invocations = !directive_invocations.is_empty();

        let mut input_sets = Vec::new();
        if has_desc {
            let desc = desc.as_ref().expect("checked desc");
            input_sets.push(quote! {
                input_value.description = ::std::option::Option::Some(::std::string::ToString::to_string(#desc));
            });
        }
        if let Some(schema_default) = schema_default {
            input_sets.push(quote!(input_value.default_value = #schema_default;));
        }
        if has_deprecation {
            input_sets.push(quote!(input_value.deprecation = #deprecation_expr;));
        }
        if has_visible {
            input_sets.push(quote!(input_value.visible = #visible;));
        }
        if inaccessible {
            input_sets.push(quote!(input_value.inaccessible = true;));
        }
        if has_tags {
            input_sets.push(quote!(input_value.tags = ::std::vec![ #(#tags),* ];));
        }
        if secret {
            input_sets.push(quote!(input_value.is_secret = true;));
        }
        if has_directive_invocations {
            input_sets.push(
                quote!(input_value.directive_invocations = ::std::vec![ #(#directive_invocations),* ];),
            );
        }

        let input_type = inferred_input_type(&crate_name, ty, field.input_using.as_ref());
        schema_fields.push(if schema_default.is_some() {
            // When a field has a default value, it should be optional in SDL
            // (nullable). Strip the trailing '!' from the type string so that,
            // e.g., `Int!` becomes `Int`, `[[Int!]!]!` becomes `[[Int!]!]`.
            quote! {
                {
                    let mut input_value = #crate_name::registry::MetaInputValue::new(
                        ::std::string::ToString::to_string(#name),
                        ::std::string::String::new(),
                    );
                    let __raw_ty = #input_type;
                    input_value.ty = if __raw_ty.ends_with('!') {
                        ::std::string::ToString::to_string(&__raw_ty[..__raw_ty.len() - 1])
                    } else {
                        __raw_ty
                    };
                    #(#input_sets)*
                    fields.insert(::std::borrow::ToOwned::to_owned(#name), input_value);
                }
            }
        } else {
            quote! {
                {
                    let mut input_value = #crate_name::registry::MetaInputValue::new(
                        ::std::string::ToString::to_string(#name),
                        #input_type,
                    );
                    #(#input_sets)*
                    fields.insert(::std::borrow::ToOwned::to_owned(#name), input_value);
                }
            }
        })
    }

    if get_fields.is_empty() {
        return Err(Error::new_spanned(
            ident,
            "A GraphQL Input Object type must define one or more input fields.",
        )
        .into());
    }

    let visible = visible_fn(&object_args.visible);

    let get_federation_fields = {
        quote! {
            let mut res = ::std::vec::Vec::new();
            #(#federation_fields)*
            ::std::option::Option::Some(::std::format!("{{ {} }}", res.join(" ")))
        }
    };

    let obj_validator = object_args
        .validator
        .as_ref()
        .map(|expr| quote! { #crate_name::CustomValidator::check(&#expr, &obj)?; });

    let mut input_object_builder = Vec::new();
    input_object_builder.push(quote!(.rust_typename(::std::any::type_name::<Self>())));
    if has_desc {
        input_object_builder.push(quote!(.description(#desc)));
    }
    if object_args.visible.is_some() {
        input_object_builder.push(quote!(.visible(#visible)));
    }
    if object_args.inaccessible {
        input_object_builder.push(quote!(.inaccessible(true)));
    }
    if !object_args.tags.is_empty() {
        input_object_builder.push(quote!(.tags(::std::vec![ #(#tags),* ])));
    }
    if !object_args.directives.is_empty() {
        input_object_builder.push(quote!(.directive_invocations(::std::vec![ #(#directives),* ])));
    }

    let expanded = if object_args.concretes.is_empty() {
        quote! {
            #[allow(clippy::all, clippy::pedantic)]
            impl #impl_generics #crate_name::InputType for #ident #ty_generics #where_clause {
                type RawValueType = Self;

                fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                    #gql_typename
                }

                fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                    registry.create_input_type::<Self, _>(#crate_name::registry::MetaTypeId::InputObject, |registry| {
                        #crate_name::registry::InputObjectBuilder::new(
                            #gql_typename_string,
                            {
                                let mut fields = #crate_name::indexmap::IndexMap::new();
                                #(#schema_fields)*
                                fields
                            },
                        )
                        #(#input_object_builder)*
                        .build()
                    })
                }

                fn parse(value: ::std::option::Option<#crate_name::Value>) -> #crate_name::InputValueResult<Self> {
                    if let ::std::option::Option::Some(#crate_name::Value::Object(obj)) = value {
                        #(#get_fields)*
                        let obj = Self { #(#fields),* };
                        #obj_validator
                        ::std::result::Result::Ok(obj)
                    } else {
                        ::std::result::Result::Err(#crate_name::InputValueError::expected_type(value.unwrap_or_default()))
                    }
                }

                fn to_value(&self) -> #crate_name::Value {
                    let mut map = #crate_name::indexmap::IndexMap::new();
                    #(#put_fields)*
                    #crate_name::Value::Object(map)
                }

                fn federation_fields() -> ::std::option::Option<::std::string::String> {
                    #get_federation_fields
                }

                fn as_raw_value(&self) -> ::std::option::Option<&Self::RawValueType> {
                    ::std::option::Option::Some(self)
                }
            }

            impl #impl_generics #crate_name::InputObjectType for #ident #ty_generics #where_clause {}
        }
    } else {
        let mut code = Vec::new();

        code.push(quote! {
            #[allow(clippy::all, clippy::pedantic)]
            impl #impl_generics #ident #ty_generics #where_clause {
                fn __internal_create_type_info_input_object(registry: &mut #crate_name::registry::Registry, name: &str) -> ::std::string::String where Self: #crate_name::InputType {
                    registry.create_input_type::<Self, _>(#crate_name::registry::MetaTypeId::InputObject, |registry| {
                        #crate_name::registry::InputObjectBuilder::new(
                            ::std::string::ToString::to_string(name),
                            {
                                let mut fields = #crate_name::indexmap::IndexMap::new();
                                #(#schema_fields)*
                                fields
                            },
                        )
                        #(#input_object_builder)*
                        .build()
                    })
                }

                fn __internal_parse(value: ::std::option::Option<#crate_name::Value>) -> #crate_name::InputValueResult<Self> where Self: #crate_name::InputType {
                    if let ::std::option::Option::Some(#crate_name::Value::Object(obj)) = value {
                        #(#get_fields)*
                        let obj = Self { #(#fields),* };
                        #obj_validator
                        ::std::result::Result::Ok(obj)
                    } else {
                        ::std::result::Result::Err(#crate_name::InputValueError::expected_type(value.unwrap_or_default()))
                    }
                }

                fn __internal_to_value(&self) -> #crate_name::Value where Self: #crate_name::InputType {
                    let mut map = #crate_name::indexmap::IndexMap::new();
                    #(#put_fields)*
                    #crate_name::Value::Object(map)
                }

                fn __internal_federation_fields() -> ::std::option::Option<::std::string::String> where Self: #crate_name::InputType {
                    #get_federation_fields
                }
            }
        });

        for concrete in &object_args.concretes {
            let gql_typename = concrete.input_name.as_ref().unwrap_or(&concrete.name);
            let params = &concrete.params.0;
            let concrete_type = quote! { #ident<#(#params),*> };

            let def_bounds = if !concrete.bounds.0.is_empty() {
                let bounds = concrete.bounds.0.iter().map(|b| quote!(#b));
                Some(quote!(<#(#bounds),*>))
            } else {
                None
            };

            let expanded = quote! {
                #[allow(clippy::all, clippy::pedantic)]
                impl #def_bounds #crate_name::InputType for #concrete_type {
                    type RawValueType = Self;

                    fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                        ::std::borrow::Cow::Borrowed(#gql_typename)
                    }

                    fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                        Self::__internal_create_type_info_input_object(registry, #gql_typename)
                    }

                    fn parse(value: ::std::option::Option<#crate_name::Value>) -> #crate_name::InputValueResult<Self> {
                        Self::__internal_parse(value)
                    }

                    fn to_value(&self) -> #crate_name::Value {
                        self.__internal_to_value()
                    }

                    fn federation_fields() -> ::std::option::Option<::std::string::String> {
                        Self::__internal_federation_fields()
                    }

                    fn as_raw_value(&self) -> ::std::option::Option<&Self::RawValueType> {
                        ::std::option::Option::Some(self)
                    }
                }

                impl #def_bounds #crate_name::InputObjectType for #concrete_type {}
            };
            code.push(expanded);
        }
        quote!(#(#code)*)
    };

    Ok(expanded.into())
}
