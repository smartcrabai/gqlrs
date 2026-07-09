use std::{borrow::Cow, collections::BTreeSet};

use crate::{Context,
    ContextSelectionSet,
    Error,
    InputType,
    InputValueError,
    InputValueResult,
    MaybeSend,
    OutputType,
    OutputTypeMarker,
    parser::types::Field,
    Positioned,
    registry,
    resolver_utils::resolve_list,
    Result,
    ServerResult,
    Value,};

impl<T: InputType + Ord> InputType for BTreeSet<T> {
    type RawValueType = Self;

    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("[{}]", T::qualified_type_name()))
    }

    fn qualified_type_name() -> String {
        format!("[{}]!", T::qualified_type_name())
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        let ty = T::create_type_info(registry);
        format!("[{ty}]!")
    }

    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        match value.unwrap_or_default() {
            Value::List(values) => values
                .into_iter()
                .map(|value| InputType::parse(Some(value)))
                .collect::<Result<_, _>>()
                .map_err(InputValueError::propagate),
            value => Ok({
                let mut result = Self::default();
                result.insert(InputType::parse(Some(value)).map_err(InputValueError::propagate)?);
                result
            }),
        }
    }

    fn to_value(&self) -> Value {
        Value::List(self.iter().map(InputType::to_value).collect())
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(self)
    }

    #[allow(clippy::manual_async_fn)]
    fn validate_input_guards<'a>(
        &'a self,
        ctx: &'a Context<'_>,
        input_value: Option<&'a Value>,
    ) -> impl std::future::Future<Output = Result<()>> + MaybeSend + 'a {
        async move {
            let values = match input_value {
                Some(Value::List(values)) => values.as_slice(),
                Some(value) => std::slice::from_ref(value),
                None => &[],
            };
            for value in values {
                let item = T::parse(Some(value.clone()))
                    .map_err(|err| Error::new(err.into_server_error(Default::default()).message))?;
                item.validate_input_guards(ctx, Some(value)).await?;
            }
            Ok(())
        }
    }
}

impl<T: OutputTypeMarker + Ord> OutputTypeMarker for BTreeSet<T> {
    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("[{}]", T::qualified_type_name()))
    }

    fn qualified_type_name() -> String {
        format!("[{}]!", T::qualified_type_name())
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        T::create_type_info(registry);
        Self::qualified_type_name()
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + Ord> OutputType for BTreeSet<T> {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        resolve_list(ctx, field, self, Some(self.len())).await
    }
}
