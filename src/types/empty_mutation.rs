use std::borrow::Cow;

use crate::{
    Context, ContextSelectionSet, ObjectType, OutputType, OutputTypeMarker, Positioned,
    ServerError, ServerResult, Value, parser::types::Field, registry, registry::MetaTypeId,
    resolver_utils::ContainerType,
};

/// Empty mutation
///
/// Only the parameters used to construct the Schema, representing an
/// unconfigured mutation.
///
/// # Examples
///
/// ```rust
/// use gqlrs::*;
///
/// struct Query;
///
/// #[Object]
/// impl Query {
///     async fn value(&self) -> i32 {
///         // A GraphQL Object type must define one or more fields.
///         100
///     }
/// }
///
/// let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
/// ```
#[derive(Default, Copy, Clone)]
pub struct EmptyMutation;

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl ContainerType for EmptyMutation {
    fn is_empty() -> bool {
        true
    }

    async fn resolve_field(&self, _ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        Ok(None)
    }
}

impl OutputTypeMarker for EmptyMutation {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("EmptyMutation")
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        registry.create_output_type::<Self, _>(MetaTypeId::Object, |_| registry::MetaType::Object {
            name: "EmptyMutation".to_string(),
            description: None,
            fields: Default::default(),
            cache_control: Default::default(),
            extends: false,
            shareable: false,
            resolvable: true,
            keys: None,
            visible: None,
            inaccessible: false,
            interface_object: false,
            tags: Default::default(),
            is_subscription: false,
            rust_typename: Some(std::any::type_name::<Self>()),
            directive_invocations: Default::default(),
            requires_scopes: Default::default(),
        })
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl OutputType for EmptyMutation {
    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        Err(ServerError::new(
            "Schema is not configured for mutations.",
            None,
        ))
    }
}

impl ObjectType for EmptyMutation {}
