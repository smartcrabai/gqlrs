use crate::{InputValueResult, MaybeSend, Value};

/// A GraphQL scalar.
///
/// You can implement the trait to create a custom scalar.
///
/// # Examples
///
/// ```rust
/// use gqlrs::*;
///
/// struct MyInt(i32);
///
/// #[Scalar]
/// impl ScalarType for MyInt {
///     fn parse(value: Value) -> InputValueResult<Self> {
///         if let Value::Number(n) = &value {
///             if let Some(n) = n.as_i64() {
///                 return Ok(MyInt(n as i32));
///             }
///         }
///         Err(InputValueError::expected_type(value))
///     }
///
///     fn to_value(&self) -> Value {
///         Value::Number(self.0.into())
///     }
/// }
/// ```
pub trait ScalarType: Sized + MaybeSend {
    /// Parse a scalar value.
    fn parse(value: Value) -> InputValueResult<Self>;

    /// Checks for a valid scalar value.
    ///
    /// Implementing this function can find incorrect input values during the
    /// verification phase, which can improve performance.
    fn is_valid(_value: &Value) -> bool {
        true
    }

    /// Convert the scalar to `Value`.
    fn to_value(&self) -> Value;
}

#[doc(hidden)]
pub fn default_scalar_name(type_name: &'static str) -> ::std::borrow::Cow<'static, str> {
    let mut name = ::std::string::String::new();
    let bytes = type_name.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if is_scalar_name_token_char(bytes[pos]) {
            let start = pos;
            pos += 1;
            while pos < bytes.len() && is_scalar_name_token_char(bytes[pos]) {
                pos += 1;
            }

            let token = &type_name[start..pos];
            let mut next = pos;
            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                next += 1;
            }

            if next + 1 < bytes.len() && bytes[next] == b':' && bytes[next + 1] == b':' {
                continue;
            }

            if is_ignored_scalar_name_token(token) {
                continue;
            }

            if name.is_empty() && !is_graphql_name_start(bytes[start]) {
                name.push('_');
            }
            name.push_str(token);
        } else {
            pos += 1;
        }
    }

    if name.is_empty() {
        name.push('_');
    }

    if name == type_name && is_valid_graphql_name(type_name) {
        ::std::borrow::Cow::Borrowed(type_name)
    } else {
        ::std::borrow::Cow::Owned(name)
    }
}

fn is_scalar_name_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_graphql_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_valid_graphql_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    is_graphql_name_start(first) && bytes.all(is_scalar_name_token_char)
}

fn is_ignored_scalar_name_token(token: &str) -> bool {
    matches!(token, "as" | "const" | "dyn" | "for" | "impl" | "mut")
}

/// Define a scalar
///
/// If your type implemented `serde::Serialize` and `serde::Deserialize`, then
/// you can use this macro to define a scalar more simply. It helps you
/// implement the `ScalarType::parse` and `ScalarType::to_value` functions by
/// calling the [from_value](fn.from_value.html) and
/// [to_value](fn.to_value.html) functions.
///
/// # Examples
///
/// ```rust
/// use gqlrs::*;
/// use serde::{Serialize, Deserialize};
/// use std::collections::HashMap;
///
/// #[derive(Serialize, Deserialize)]
/// struct MyValue {
///     a: i32,
///     b: HashMap<String, i32>,
/// }
///
/// scalar!(MyValue);
///
/// // Rename to `MV`.
/// // scalar!(MyValue, "MV");
///
/// // Rename to `MV` and add description.
/// // scalar!(MyValue, "MV", "This is my value");
///
/// // Rename to `MV`, add description and specifiedByURL.
/// // scalar!(MyValue, "MV", "This is my value", "https://tools.ietf.org/html/rfc4122");
///
/// struct Query;
///
/// #[Object]
/// impl Query {
///     async fn value(&self, input: MyValue) -> MyValue {
///         input
///     }
/// }
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async move {
/// let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
/// let res = schema.execute(r#"{ value(input: {a: 10, b: {v1: 1, v2: 2} }) }"#).await.into_result().unwrap().data;
/// assert_eq!(res, value!({
///     "value": {
///         "a": 10,
///         "b": {"v1": 1, "v2": 2},
///     }
/// }));
/// # });
/// ```
#[macro_export]
macro_rules! scalar {
    ($ty:ty, $name:literal, $desc:literal, $specified_by_url:literal) => {
        $crate::scalar_internal!(
            $ty,
            ::std::borrow::Cow::Borrowed($name),
            ::std::option::Option::Some(::std::string::ToString::to_string($desc)),
            ::std::option::Option::Some(::std::string::ToString::to_string($specified_by_url))
        );
    };

    ($ty:ty, $name:literal, $desc:literal) => {
        $crate::scalar_internal!(
            $ty,
            ::std::borrow::Cow::Borrowed($name),
            ::std::option::Option::Some(::std::string::ToString::to_string($desc)),
            ::std::option::Option::None
        );
    };

    ($ty:ty, $name:literal) => {
        $crate::scalar_internal!(
            $ty,
            ::std::borrow::Cow::Borrowed($name),
            ::std::option::Option::None,
            ::std::option::Option::None
        );
    };

    ($ty:ty) => {
        $crate::scalar_internal!(
            $ty,
            $crate::resolver_utils::default_scalar_name(::std::stringify!($ty)),
            ::std::option::Option::None,
            ::std::option::Option::None
        );
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal {
    ($ty:ty, $name:expr, $desc:expr, $specified_by_url:expr) => {
        impl $crate::ScalarType for $ty {
            fn parse(value: $crate::Value) -> $crate::InputValueResult<Self> {
                ::std::result::Result::Ok($crate::from_value(value)?)
            }

            fn to_value(&self) -> $crate::Value {
                $crate::to_value(self).unwrap_or_else(|_| $crate::Value::Null)
            }
        }

        impl $crate::InputType for $ty {
            type RawValueType = Self;

            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                $name
            }

            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_input_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::Cow::into_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),
                        visible: ::std::option::Option::None,
                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }

            fn parse(
                value: ::std::option::Option<$crate::Value>,
            ) -> $crate::InputValueResult<Self> {
                <$ty as $crate::ScalarType>::parse(value.unwrap_or_default())
            }

            fn to_value(&self) -> $crate::Value {
                <$ty as $crate::ScalarType>::to_value(self)
            }

            fn as_raw_value(&self) -> ::std::option::Option<&Self::RawValueType> {
                ::std::option::Option::Some(self)
            }
        }

        $crate::scalar_internal_output!($ty, $name, $desc, $specified_by_url);
    };
}

#[cfg(all(feature = "boxed-trait", not(feature = "no_send")))]
#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal_output {
    ($ty:ty, $name:expr, $desc:expr, $specified_by_url:expr) => {
        impl $crate::OutputTypeMarker for $ty {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                $name
            }

            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_output_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::Cow::into_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),
                        visible: ::std::option::Option::None,
                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }
        }

        #[$crate::async_trait::async_trait]
        impl $crate::OutputType for $ty {
            async fn resolve(
                &self,
                _: &$crate::ContextSelectionSet<'_>,
                _field: &$crate::Positioned<$crate::parser::types::Field>,
            ) -> $crate::ServerResult<$crate::Value> {
                ::std::result::Result::Ok($crate::ScalarType::to_value(self))
            }
        }
    };
}

#[cfg(all(feature = "boxed-trait", feature = "no_send"))]
#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal_output {
    ($ty:ty, $name:expr, $desc:expr, $specified_by_url:expr) => {
        #[$crate::async_trait::async_trait(?Send)]
        impl $crate::OutputType for $ty {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                ::std::borrow::Cow::Borrowed($name)
            }

            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_output_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::ToOwned::to_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),
                        visible: ::std::option::Option::None,
                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }

            async fn resolve(
                &self,
                _: &$crate::ContextSelectionSet<'_>,
                _field: &$crate::Positioned<$crate::parser::types::Field>,
            ) -> $crate::ServerResult<$crate::Value> {
                ::std::result::Result::Ok($crate::ScalarType::to_value(self))
            }
        }
    };
}

#[cfg(not(feature = "boxed-trait"))]
#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal_output {
    ($ty:ty, $name:expr, $desc:expr, $specified_by_url:expr) => {
        impl $crate::OutputTypeMarker for $ty {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                $name
            }

            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_output_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::Cow::into_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),
                        visible: ::std::option::Option::None,
                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }
        }

        impl $crate::OutputType for $ty {
            async fn resolve(
                &self,
                _: &$crate::ContextSelectionSet<'_>,
                _field: &$crate::Positioned<$crate::parser::types::Field>,
            ) -> $crate::ServerResult<$crate::Value> {
                ::std::result::Result::Ok($crate::ScalarType::to_value(self))
            }
        }
    };
}
