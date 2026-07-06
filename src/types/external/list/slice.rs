use std::{borrow::Cow, sync::Arc};

use crate::{
    Context, ContextSelectionSet, InputType, InputValueError, InputValueResult, OutputType,
    OutputTypeMarker, Positioned, Result, ServerResult, Value, parser::types::Field, registry,
    resolver_utils::resolve_list,
};

impl<'a, T: OutputTypeMarker + 'a> OutputTypeMarker for &'a [T] {
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

#[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
impl<'a, T: OutputType + 'a> OutputType for &'a [T] {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        resolve_list(ctx, field, self.iter(), Some(self.len())).await
    }
}

macro_rules! impl_output_marker_slice_for_smart_ptr {
    ($ty:ty) => {
        impl<T: OutputTypeMarker> OutputTypeMarker for $ty {
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
    };
}

macro_rules! impl_output_slice_for_smart_ptr {
    ($ty:ty) => {
        #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
        impl<T: OutputType> OutputType for $ty {
            async fn resolve(
                &self,
                ctx: &ContextSelectionSet<'_>,
                field: &Positioned<Field>,
            ) -> ServerResult<Value> {
                resolve_list(ctx, field, self.iter(), Some(self.len())).await
            }
        }
    };
}

impl_output_marker_slice_for_smart_ptr!(Box<[T]>);
impl_output_marker_slice_for_smart_ptr!(Arc<[T]>);
impl_output_slice_for_smart_ptr!(Box<[T]>);
impl_output_slice_for_smart_ptr!(Arc<[T]>);

macro_rules! impl_input_slice_for_smart_ptr {
    ($ty:ty) => {
        impl<T: InputType> InputType for $ty {
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
                    value => {
                        Ok(
                            vec![
                                InputType::parse(Some(value))
                                    .map_err(InputValueError::propagate)?,
                            ]
                            .into(),
                        )
                    }
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
            ) -> impl std::future::Future<Output = Result<()>> + Send + 'a {
                async move {
                    if let Some(Value::List(values)) = input_value {
                        for (item, value) in self.iter().zip(values) {
                            item.validate_input_guards(ctx, Some(value)).await?;
                        }
                    } else if let Some(value) = input_value
                        && let Some(item) = self.first()
                    {
                        item.validate_input_guards(ctx, Some(value)).await?;
                    }
                    Ok(())
                }
            }
        }
    };
}

impl_input_slice_for_smart_ptr!(Box<[T]>);
impl_input_slice_for_smart_ptr!(Arc<[T]>);
