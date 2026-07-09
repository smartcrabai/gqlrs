use std::{
    borrow::Cow,
    future::Future,
    sync::{Arc, Weak},
};

use async_graphql_value::ConstValue;

use crate::{
    ContainerType, Context, ContextSelectionSet, Error, InputValueError, InputValueResult,
    MaybeSend, MaybeSync, Positioned, Result, ServerResult, Value,
    parser::types::Field,
    registry::{self, Registry, SemanticNullability},
};

#[doc(hidden)]
pub trait Description {
    fn description() -> Cow<'static, str>;
}

/// Used to specify the GraphQL Type name.
pub trait TypeName: MaybeSend + MaybeSync {
    /// Returns a GraphQL type name.
    fn type_name() -> Cow<'static, str>;
}

/// Represents a GraphQL input type.
pub trait InputType: MaybeSend + MaybeSync + Sized {
    /// The raw type used for validator.
    ///
    /// Usually it is `Self`, but the wrapper type is its internal type.
    ///
    /// For example:
    ///
    /// `i32::RawValueType` is `i32`
    /// `Option<i32>::RawValueType` is `i32`.
    type RawValueType: ?Sized;

    /// Type the name.
    fn type_name() -> Cow<'static, str>;

    /// Qualified typename.
    fn qualified_type_name() -> String {
        format!("{}!", Self::type_name())
    }

    /// Create type information in the registry and return qualified typename.
    fn create_type_info(registry: &mut registry::Registry) -> String;

    /// Parse from `Value`. None represents undefined.
    fn parse(value: Option<Value>) -> InputValueResult<Self>;

    /// Convert to a `Value` for introspection.
    fn to_value(&self) -> Value;

    /// Get the federation fields, only for InputObject.
    #[doc(hidden)]
    fn federation_fields() -> Option<String> {
        None
    }

    /// Returns a reference to the raw value.
    fn as_raw_value(&self) -> Option<&Self::RawValueType>;

    /// Validates field guards on this input type.
    ///
    /// This method is called after parsing the input to check any field-level
    /// guards that require context (e.g., role-based access). The `value`
    /// argument is the resolved input value supplied by the client, if any, so
    /// implementors can distinguish omitted fields from provided fields. By
    /// default, this is a no-op. InputObject types override this to check field
    /// guards.
    fn validate_input_guards<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
        _value: Option<&'a Value>,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        async { Ok(()) }
    }
}

/// Marker trait for GraphQL output type metadata.
///
/// This trait contains the type-level metadata methods that were previously
/// on `OutputType`. Splitting them out reduces monomorphization when
/// `ContainerType` is used, leading to faster compile times.
pub trait OutputTypeMarker: MaybeSend + MaybeSync {
    /// Type the name.
    fn type_name() -> Cow<'static, str>;

    /// Qualified typename.
    fn qualified_type_name() -> String {
        format!("{}!", Self::type_name())
    }

    /// Introspection type name
    ///
    /// Is the return value of field `__typename`, the interface and union
    /// should return the current type, and the others return `Type::type_name`.
    fn introspection_type_name(&self) -> Cow<'static, str> {
        Self::type_name()
    }

    /// Semantic nullability of this type.
    ///
    /// When set to something other than `SemanticNullability::None`, the field
    /// will be annotated with the `@semanticNonNull` directive in SDL exports.
    fn semantic_nullability() -> SemanticNullability {
        SemanticNullability::None
    }

    /// Create type information in the registry and return qualified typename.
    fn create_type_info(registry: &mut registry::Registry) -> String;
}

/// Represents a GraphQL output type.
#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
pub trait OutputType: OutputTypeMarker {
    /// Resolve an output value to `gqlrs::Value`.
    #[cfg(feature = "boxed-trait")]
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value>;

    /// Resolve an output value to `gqlrs::Value`.
    #[cfg(not(feature = "boxed-trait"))]
    fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> impl Future<Output = ServerResult<Value>> + MaybeSend;
}

impl<T: OutputTypeMarker + ?Sized> OutputTypeMarker for &T {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn semantic_nullability() -> SemanticNullability {
        T::semantic_nullability()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + ?Sized> OutputType for &T {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        T::resolve(*self, ctx, field).await
    }
}

impl<T: OutputTypeMarker + MaybeSync, E: Into<Error> + MaybeSend + MaybeSync + Clone>
    OutputTypeMarker for Result<T, E>
{
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn qualified_type_name() -> String {
        T::type_name().to_string()
    }

    fn semantic_nullability() -> SemanticNullability {
        match T::semantic_nullability() {
            SemanticNullability::None => SemanticNullability::OutNonNull,
            SemanticNullability::OutNonNull => SemanticNullability::OutNonNull,
            SemanticNullability::InNonNull => SemanticNullability::BothNonNull,
            SemanticNullability::BothNonNull => SemanticNullability::BothNonNull,
        }
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry);
        T::type_name().to_string()
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + MaybeSync, E: Into<Error> + MaybeSend + MaybeSync + Clone> OutputType
    for Result<T, E>
{
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        match self {
            Ok(value) => match OutputType::resolve(value, ctx, field).await {
                Ok(value) => Ok(value),
                Err(err) => {
                    ctx.add_error(crate::resolver_utils::set_error_path_if_empty(ctx, err));
                    Ok(Value::Null)
                }
            },
            Err(err) => {
                let err = err.clone().into().into_server_error(field.pos);
                ctx.add_error(ctx.set_error_path(err));
                Ok(Value::Null)
            }
        }
    }
}

/// A GraphQL object.
pub trait ObjectType: ContainerType + OutputType {}

impl<T: ObjectType> ObjectType for Result<T> {}

impl<T: ObjectType + ?Sized> ObjectType for &T {}

impl<T: ObjectType + ?Sized> ObjectType for Box<T> {}

impl<T: ObjectType + ?Sized> ObjectType for Arc<T> {}

/// A GraphQL interface.
pub trait InterfaceType: ContainerType {}

/// A GraphQL interface.
pub trait UnionType: ContainerType {}

/// A GraphQL input object.
pub trait InputObjectType: InputType {}

/// A GraphQL oneof input object.
pub trait OneofObjectType: InputObjectType {}

impl<T: OutputTypeMarker + ?Sized> OutputTypeMarker for Box<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn semantic_nullability() -> SemanticNullability {
        T::semantic_nullability()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + ?Sized> OutputType for Box<T> {
    #[cfg(feature = "boxed-trait")]
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        T::resolve(self.as_ref(), ctx, field).await
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[cfg(not(feature = "boxed-trait"))]
    fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> impl Future<Output = ServerResult<Value>> + MaybeSend {
        T::resolve(self.as_ref(), ctx, field)
    }
}

impl<T: InputType> InputType for Box<T> {
    type RawValueType = T::RawValueType;

    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }

    fn parse(value: Option<ConstValue>) -> InputValueResult<Self> {
        T::parse(value)
            .map(Box::new)
            .map_err(InputValueError::propagate)
    }

    fn to_value(&self) -> ConstValue {
        T::to_value(&self)
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        self.as_ref().as_raw_value()
    }

    fn validate_input_guards<'a>(
        &'a self,
        ctx: &'a Context<'_>,
        value: Option<&'a Value>,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        self.as_ref().validate_input_guards(ctx, value)
    }
}

impl<T: OutputTypeMarker + ?Sized> OutputTypeMarker for Arc<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn semantic_nullability() -> SemanticNullability {
        T::semantic_nullability()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + ?Sized> OutputType for Arc<T> {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        T::resolve(&**self, ctx, field).await
    }
}

impl<T: InputType> InputType for Arc<T> {
    type RawValueType = T::RawValueType;

    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }

    fn parse(value: Option<ConstValue>) -> InputValueResult<Self> {
        T::parse(value)
            .map(Arc::new)
            .map_err(InputValueError::propagate)
    }

    fn to_value(&self) -> ConstValue {
        T::to_value(&self)
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        self.as_ref().as_raw_value()
    }

    fn validate_input_guards<'a>(
        &'a self,
        ctx: &'a Context<'_>,
        value: Option<&'a Value>,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        self.as_ref().validate_input_guards(ctx, value)
    }
}

impl<T: OutputTypeMarker + ?Sized> OutputTypeMarker for Weak<T> {
    fn type_name() -> Cow<'static, str> {
        <Option<Arc<T>> as OutputTypeMarker>::type_name()
    }

    fn semantic_nullability() -> SemanticNullability {
        T::semantic_nullability()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        <Option<Arc<T>> as OutputTypeMarker>::create_type_info(registry)
    }
}

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl<T: OutputType + ?Sized> OutputType for Weak<T> {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        self.upgrade().resolve(ctx, field).await
    }
}

#[doc(hidden)]
#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
pub trait ComplexObject {
    fn fields(registry: &mut registry::Registry) -> Vec<(String, registry::MetaField)>;

    #[cfg(feature = "boxed-trait")]
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>>;

    #[cfg(not(feature = "boxed-trait"))]
    fn resolve_field(
        &self,
        ctx: &Context<'_>,
    ) -> impl Future<Output = ServerResult<Option<Value>>> + MaybeSend;
}
