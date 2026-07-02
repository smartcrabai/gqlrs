use std::borrow::Cow;

use crate::{Context,
    ContextSelectionSet,
    InputType,
    InputValueError,
    InputValueResult,
    MaybeSend,
    MaybeSync,
    OutputType,
    OutputTypeMarker,
    parser::types::Field,
    Positioned,
    registry,
    Result,
    ServerResult,
    Value,};

impl<T: InputType> InputType for Option<T> {
    type RawValueType = T::RawValueType;

    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn qualified_type_name() -> String {
        T::type_name().to_string()
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        let ty = T::create_type_info(registry);
        registry::MetaTypeName::create(&ty)
            .unwrap_non_null()
            .to_string()
    }

    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        match value.unwrap_or_default() {
            Value::Null => Ok(None),
            value => Ok(Some(
                T::parse(Some(value)).map_err(InputValueError::propagate)?,
            )),
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Some(value) => value.to_value(),
            None => Value::Null,
        }
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        match self {
            Some(value) => value.as_raw_value(),
            None => None,
        }
    }

    fn validate_input_guards<'a>(
        &'a self,
        ctx: &'a Context<'_>,
        input_value: Option<&'a Value>,
    ) -> impl std::future::Future<Output = Result<()>> + MaybeSend + 'a {
        Box::pin(async move {
            if let Some(value) = self {
                value.validate_input_guards(ctx, input_value).await?;
            }
            Ok(())
        })
    }
}

impl<T: OutputTypeMarker + MaybeSync> OutputTypeMarker for Option<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn qualified_type_name() -> String {
        T::type_name().to_string()
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        let ty = T::create_type_info(registry);
        registry::MetaTypeName::create(&ty)
            .unwrap_non_null()
            .to_string()
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + MaybeSync> OutputType for Option<T> {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        if let Some(inner) = self {
            match OutputType::resolve(inner, ctx, field).await {
                Ok(value) => Ok(value),
                Err(err) => {
                    ctx.add_error(err);
                    Ok(Value::Null)
                }
            }
        } else {
            Ok(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::InputType;

    #[test]
    fn test_optional_type() {
        assert_eq!(Option::<i32>::type_name(), "Int");
        assert_eq!(Option::<i32>::qualified_type_name(), "Int");
        assert_eq!(&Option::<i32>::type_name(), "Int");
        assert_eq!(&Option::<i32>::qualified_type_name(), "Int");
    }
}
