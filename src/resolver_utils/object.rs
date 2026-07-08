use std::{future::Future, pin::Pin};

use indexmap::IndexMap;

use crate::{Context, Error, Name, OutputType, ServerError, ServerResult, Value};

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
) -> Pin<Box<dyn Future<Output = ServerResult<Option<Value>>> + Send + 'a>>
where
    T: OutputType + Send,
    E: Into<Error> + Send + Sync,
    F: Future<Output = Result<T, E>> + Send + 'a,
{
    Box::pin(async move {
        let obj: T = fut.await.map_err(|err| {
            let err = ::std::convert::Into::<Error>::into(err).into_server_error(ctx.item.pos);
            ctx.set_error_path(err)
        })?;

        let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
        OutputType::resolve(&obj, &ctx_obj, ctx.item)
            .await
            .map(Option::Some)
    })
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
) -> Pin<Box<dyn Future<Output = ServerResult<Option<Value>>> + Send + 'a>> {
    Box::pin(async move {
        OutputType::resolve(
            value,
            &ctx.with_selection_set(&ctx.item.node.selection_set),
            ctx.item,
        )
        .await
        .map(Option::Some)
        .map_err(|err| ctx.set_error_path(err))
    })
}
