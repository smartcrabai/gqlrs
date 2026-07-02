use std::borrow::Cow;

use crate::{
    Context, ContextDirective, MaybeSend, MaybeSync, ServerResult, Value, extensions::ResolveFut,
    parser::types::Directive, registry::Registry,
};

#[doc(hidden)]
pub trait CustomDirectiveFactory: MaybeSend + MaybeSync + 'static {
    fn name(&self) -> Cow<'static, str>;

    fn register(&self, registry: &mut Registry);

    fn create(
        &self,
        ctx: &ContextDirective<'_>,
        directive: &Directive,
    ) -> ServerResult<Box<dyn CustomDirective>>;
}

#[doc(hidden)]
// minimal amount required to register directive into registry
pub trait TypeDirective {
    fn name(&self) -> Cow<'static, str>;

    fn register(&self, registry: &mut Registry);
}

/// Represents a custom directive.
#[cfg_attr(not(feature = "no_send"), async_trait::async_trait)]
#[cfg_attr(feature = "no_send", async_trait::async_trait(?Send))]
#[allow(unused_variables)]
pub trait CustomDirective: MaybeSync + MaybeSend + 'static {
    /// Called at resolve field.
    async fn resolve_field(
        &self,
        ctx: &Context<'_>,
        resolve: ResolveFut<'_>,
    ) -> ServerResult<Option<Value>> {
        resolve.await
    }
}
