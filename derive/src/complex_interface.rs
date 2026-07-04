use proc_macro::TokenStream;
use quote::quote;
use syn::{Block, Error, ImplItem, ItemImpl, ReturnType, ext::IdentExt};

use crate::{
    args::{self, RenameRuleExt, RenameTarget},
    output_type::OutputType,
    utils::{
        GeneratorResult, extract_input_args, gen_boxed_trait, generate_default, get_crate_path,
        get_type_path_and_name, parse_graphql_attrs, remove_graphql_attrs,
    },
};

pub fn generate(
    interface_args: &args::ComplexInterface,
    item_impl: &mut ItemImpl,
) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&interface_args.crate_path, interface_args.internal);
    let boxed_trait = gen_boxed_trait(&crate_name);
    let (self_ty, _) = get_type_path_and_name(item_impl.self_ty.as_ref())?;
    let generics = &item_impl.generics;
    let where_clause = &item_impl.generics.where_clause;

    let mut resolvers = Vec::new();

    for item in &mut item_impl.items {
        if let ImplItem::Fn(method) = item {
            let method_args: args::ComplexInterfaceField =
                parse_graphql_attrs(&method.attrs)?.unwrap_or_default();

            if method_args.skip {
                remove_graphql_attrs(&mut method.attrs);
                continue;
            }

            let is_async = method.sig.asyncness.is_some();
            let field_name = method_args.name.clone().unwrap_or_else(|| {
                interface_args
                    .rename_fields
                    .rename(method.sig.ident.unraw().to_string(), RenameTarget::Field)
            });

            let args = extract_input_args::<args::Argument>(&crate_name, method)?;
            let mut use_params = Vec::new();
            let mut get_params = Vec::new();

            for (
                ident,
                ty,
                args::Argument {
                    name,
                    default,
                    default_with,
                    ..
                },
            ) in &args
            {
                let name = name.clone().unwrap_or_else(|| {
                    interface_args
                        .rename_args
                        .rename(ident.ident.unraw().to_string(), RenameTarget::Argument)
                });

                let default = generate_default(default, default_with)?;
                let default = match default {
                    Some(default) => {
                        quote! { ::std::option::Option::Some(|| -> #ty { #default }) }
                    }
                    None => quote! { ::std::option::Option::None },
                };

                let param_ident = &ident.ident;
                use_params.push(quote! { #param_ident });

                get_params.push(quote! {
                    #[allow(non_snake_case)]
                    let (_, #param_ident) = ctx.param_value::<#ty>(#name, #default)?;
                });
            }

            let output = method.sig.output.clone();
            let ty = match &output {
                ReturnType::Type(_, ty) => OutputType::parse(ty)?,
                ReturnType::Default => {
                    return Err(
                        Error::new_spanned(&output, "Resolver must have a return type").into(),
                    );
                }
            };

            let field_ident = &method.sig.ident;
            if is_async && let OutputType::Value(inner_ty) = &ty {
                let block = &method.block;
                let new_block = quote!({
                    {
                        ::std::result::Result::Ok(async move {
                            let value: #inner_ty = #block;
                            value
                        }.await)
                    }
                });
                method.block = syn::parse2::<Block>(new_block).expect("invalid block");
                method.sig.output =
                    syn::parse2::<ReturnType>(quote! { -> #crate_name::Result<#inner_ty> })
                        .expect("invalid result type");
            }

            let resolve_obj = if is_async {
                quote! {
                    {
                        let res = self.#field_ident(ctx, #(#use_params),*).await;
                        res.map_err(|err| ::std::convert::Into::<#crate_name::Error>::into(err).into_server_error(ctx.item.pos))
                    }
                }
            } else {
                match &ty {
                    OutputType::Value(_) => {
                        quote! {
                            ::std::result::Result::Ok(self.#field_ident(ctx, #(#use_params),*))
                        }
                    }
                    OutputType::Result(_) => {
                        quote! {
                            self.#field_ident(ctx, #(#use_params),*)
                                .map_err(|err| {
                                    ::std::convert::Into::<#crate_name::Error>::into(err)
                                        .into_server_error(ctx.item.pos)
                                })
                        }
                    }
                }
            };

            let resolve_block = if is_async {
                quote! {
                    let f = async move {
                        #(#get_params)*
                        #resolve_obj.map(::std::option::Option::Some)
                    };
                    let obj = match f.await.map_err(|err| ctx.set_error_path(err))? {
                        ::std::option::Option::Some(obj) => obj,
                        ::std::option::Option::None => {
                            return ::std::result::Result::Ok(
                                ::std::option::Option::Some(#crate_name::Value::Null),
                            );
                        }
                    };
                    let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                    return #crate_name::OutputType::resolve(&obj, &ctx_obj, ctx.item)
                        .await
                        .map(::std::option::Option::Some);
                }
            } else {
                quote! {
                    #(#get_params)*
                    let obj = #resolve_obj.map_err(|err| ctx.set_error_path(err))?;
                    return #crate_name::resolver_utils::resolve_simple_field_value(ctx, &obj).await;
                }
            };

            resolvers.push(quote! {
                if ctx.item.node.name.node == #field_name {
                    #resolve_block
                }
            });

            remove_graphql_attrs(&mut method.attrs);
        }
    }

    let expanded = quote! {
        #item_impl

        #[allow(clippy::all, clippy::pedantic)]
        #boxed_trait
        impl #generics #crate_name::ComplexObject for #self_ty #where_clause {
            fn fields(registry: &mut #crate_name::registry::Registry) -> ::std::vec::Vec<(::std::string::String, #crate_name::registry::MetaField)> {
                // ComplexInterface doesn't add new fields to the schema,
                // it only provides resolvers for existing interface fields
                ::std::vec::Vec::new()
            }

            async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                #(#resolvers)*
                ::std::result::Result::Ok(::std::option::Option::None)
            }
        }
    };

    Ok(expanded.into())
}
