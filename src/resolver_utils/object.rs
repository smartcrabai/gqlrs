use std::future::Future;

use indexmap::IndexMap;

use crate::{
    Context, ContextBase, Error, MaybeSend, MaybeSync, Name, OutputType, ServerError, ServerResult,
    Value,
    sendable::{FutureMaybeSendExt, MaybeBoxFuture},
};

/// Helper used by proc-macro-generated object resolvers to reduce emitted code.
#[doc(hidden)]
// NOTE: Boxing the future here prevents stack overflows when resolve_field
// is called many times in deeply nested or wide selection sets. Without
// boxing, the futures are allocated inline on the stack and the frame size
// grows linearly with the nesting depth, eventually overflowing the stack.
// See: https://github.com/async-graphql/async-graphql/issues/1809
#[inline(never)]
pub fn resolve_field_async<'a, T, E, F>(
    ctx: &'a Context<'a>,
    fut: F,
    nullable: bool,
) -> MaybeBoxFuture<'a, ServerResult<Option<Value>>>
where
    T: OutputType + MaybeSend,
    E: Into<Error> + MaybeSend + MaybeSync,
    F: Future<Output = Result<T, E>> + MaybeSend + 'a,
{
    (async move {
        match fut.await {
            Ok(obj) => {
                let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                match OutputType::resolve(&obj, &ctx_obj, ctx.item).await {
                    Ok(value) => Ok(Some(value)),
                    Err(err) if nullable => {
                        ctx.add_error(set_error_path_if_empty(ctx, err));
                        Ok(Some(Value::Null))
                    }
                    Err(err) => Err(err),
                }
            }
            Err(err) => {
                let err = ::std::convert::Into::<Error>::into(err).into_server_error(ctx.item.pos);
                let err = ctx.set_error_path(err);
                if nullable {
                    ctx.add_error(err);
                    Ok(Some(Value::Null))
                } else {
                    Err(err)
                }
            }
        }
    }).boxed_maybe_send()
}

#[doc(hidden)]
pub fn set_error_path_if_empty<T>(ctx: &ContextBase<'_, T>, err: ServerError) -> ServerError {
    if err.path.is_empty() {
        ctx.set_error_path(err)
    } else {
        err
    }
}

/// Helper used by proc-macro-generated object resolvers to parse entity params.
#[doc(hidden)]
pub fn find_entity_params<'a>(
    ctx: &'a Context<'a>,
    params: &'a Value,
) -> ServerResult<Option<(&'a IndexMap<Name, Value>, &'a String)>> {
    let params = match params {
        Value::Object(params) => params,
        _ => return Ok(None),
    };
    let typename = if let Some(Value::String(typename)) = params.get("__typename") {
        typename
    } else {
        return Err(ServerError::new(
            r#""__typename" must be an existing string."#,
            Some(ctx.item.pos),
        ));
    };
    Ok(Some((params, typename)))
}

/// Resolve a SimpleObject field value using the current selection set.
///
/// This is a small helper used by derive codegen to keep emitted resolver code
/// small.
#[doc(hidden)]
// NOTE: Boxing the future here prevents stack overflows when this function
// is called many times in deeply nested or wide selection sets. Without
// boxing, the futures are allocated inline on the stack and the frame size
// grows linearly with the nesting depth, eventually overflowing the stack.
// See: https://github.com/async-graphql/async-graphql/issues/1809
#[inline(never)]
pub fn resolve_simple_field_value<'a, T: OutputType + ?Sized + 'a>(
    ctx: &'a Context<'_>,
    value: &'a T,
) -> MaybeBoxFuture<'a, ServerResult<Option<Value>>> {
    (async move {
        OutputType::resolve(
            value,
            &ctx.with_selection_set(&ctx.item.node.selection_set),
            ctx.item,
        )
        .await
        .map(Option::Some)
        .map_err(|err| ctx.set_error_path(err))
    }).boxed_maybe_send()
}

/// Resolve a nullable field value, recording any resolver error and returning
/// `Value::Null` instead of propagating it past this field.
#[doc(hidden)]
pub async fn resolve_nullable_field_value<T: OutputType + ?Sized>(
    ctx: &Context<'_>,
    value: &T,
) -> ServerResult<Option<Value>> {
    match OutputType::resolve(
        value,
        &ctx.with_selection_set(&ctx.item.node.selection_set),
        ctx.item,
    )
    .await
    {
        Ok(value) => Ok(Some(value)),
        Err(err) => {
            ctx.add_error(set_error_path_if_empty(ctx, err));
            Ok(Some(Value::Null))
        }
    }
}
