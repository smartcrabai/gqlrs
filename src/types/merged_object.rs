use std::{borrow::Cow, pin::Pin};

use indexmap::IndexMap;

use crate::{
    CacheControl, ContainerType, Context, ContextSelectionSet, OutputType, OutputTypeMarker,
    Positioned, Response, ServerResult, SimpleObject, SubscriptionType, Value,
    futures_util::stream::Stream,
    parser::types::Field,
    registry::{MetaField, MetaType, MetaTypeId, Registry},
};


fn extend_fields(
    fields: &mut IndexMap<String, MetaField>,
    new_fields: IndexMap<String, MetaField>,
    merged_type: &str,
    type_name: impl Fn() -> Cow<'static, str>,
) {
    for (name, field) in new_fields {
        if fields.contains_key(&name) {
            panic!(
                "Field `{}` is defined multiple times in {} `{}`",
                name,
                merged_type,
                type_name()
            );
        }
        fields.insert(name, field);
    }
}

#[doc(hidden)]
pub struct MergedObject<A, B>(pub A, pub B);

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<A, B> ContainerType for MergedObject<A, B>
where
    A: ContainerType,
    B: ContainerType,
{
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        match self.0.resolve_field(ctx).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => self.1.resolve_field(ctx).await,
            Err(err) => Err(err),
        }
    }

    async fn find_entity(&self, ctx: &Context<'_>, params: &Value) -> ServerResult<Option<Value>> {
        match self.0.find_entity(ctx, params).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => self.1.find_entity(ctx, params).await,
            Err(err) => Err(err),
        }
    }

    async fn find_entities(
        &self,
        ctx: &Context<'_>,
        representations: &[Value],
    ) -> ServerResult<Vec<Option<Value>>> {
        let mut results = self.0.find_entities(ctx, representations).await?;
        // For any entity not found by the first resolver, try the second
        let missing_indices: Vec<usize> = results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| if r.is_none() { Some(i) } else { None })
            .collect();
        if !missing_indices.is_empty() {
            let missing_reps: Vec<Value> = missing_indices
                .iter()
                .map(|&i| representations[i].clone())
                .collect();
            let second_results: Vec<Option<Value>> =
                self.1.find_entities(ctx, &missing_reps).await?;
            for (&i, value) in missing_indices.iter().zip(second_results) {
                results[i] = value;
            }
        }
        Ok(results)
    }
}

impl<A, B> OutputTypeMarker for MergedObject<A, B>
where
    A: OutputTypeMarker,
    B: OutputTypeMarker,
{
    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("{}_{}", A::type_name(), B::type_name()))
    }

    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_output_type::<Self, _>(MetaTypeId::Object, |registry| {
            let mut fields = IndexMap::new();
            let mut cc = CacheControl::default();

            if let MetaType::Object {
                fields: b_fields,
                cache_control: b_cc,
                ..
            } = registry.create_fake_output_type::<B>()
            {
                extend_fields(&mut fields, b_fields, "MergedObject", Self::type_name);
                cc = cc.merge(&b_cc);
            }

            if let MetaType::Object {
                fields: a_fields,
                cache_control: a_cc,
                ..
            } = registry.create_fake_output_type::<A>()
            {
                extend_fields(&mut fields, a_fields, "MergedObject", Self::type_name);
                cc = cc.merge(&a_cc);
            }

            MetaType::Object {
                name: Self::type_name().to_string(),
                description: None,
                fields,
                cache_control: cc,
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
            }
        })
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<A, B> OutputType for MergedObject<A, B>
where
    A: OutputType,
    B: OutputType,
{
    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        unreachable!()
    }
}

impl<A, B> SubscriptionType for MergedObject<A, B>
where
    A: SubscriptionType,
    B: SubscriptionType,
{
    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("{}_{}", A::type_name(), B::type_name()))
    }

    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_subscription_type::<Self, _>(|registry| {
            let mut fields = IndexMap::new();
            let mut cc = CacheControl::default();

            if let MetaType::Object {
                fields: b_fields,
                cache_control: b_cc,
                ..
            } = registry.create_fake_subscription_type::<B>()
            {
                extend_fields(&mut fields, b_fields, "MergedSubscription", Self::type_name);
                cc = cc.merge(&b_cc);
            }

            if let MetaType::Object {
                fields: a_fields,
                cache_control: a_cc,
                ..
            } = registry.create_fake_subscription_type::<A>()
            {
                extend_fields(&mut fields, a_fields, "MergedSubscription", Self::type_name);
                cc = cc.merge(&a_cc);
            }

            MetaType::Object {
                name: Self::type_name().to_string(),
                description: None,
                fields,
                cache_control: cc,
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
            }
        })
    }

    #[cfg(not(feature = "no_send"))]
    fn create_field_stream<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
    ) -> Option<Pin<Box<dyn Stream<Item = Response> + Send + 'a>>> {
        unreachable!()
    }

    #[cfg(feature = "no_send")]
    fn create_field_stream<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
    ) -> Option<Pin<Box<dyn Stream<Item = Response> + 'a>>> {
        unreachable!()
    }
}

#[doc(hidden)]
#[derive(SimpleObject, Default)]
#[graphql(internal, fake)]
pub struct MergedObjectTail;

impl SubscriptionType for MergedObjectTail {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("MergedSubscriptionTail")
    }

    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_subscription_type::<Self, _>(|_| MetaType::Object {
            name: "MergedSubscriptionTail".to_string(),
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

    #[cfg(not(feature = "no_send"))]
    fn create_field_stream<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
    ) -> Option<Pin<Box<dyn Stream<Item = Response> + Send + 'a>>> {
        unreachable!()
    }

    #[cfg(feature = "no_send")]
    fn create_field_stream<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
    ) -> Option<Pin<Box<dyn Stream<Item = Response> + 'a>>> {
        unreachable!()
    }
}
