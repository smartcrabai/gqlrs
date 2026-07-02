use std::{
    borrow::Cow,
    char::ParseCharError,
    convert::Infallible,
    fmt::Display,
    num::{ParseFloatError, ParseIntError},
    ops::{Deref, DerefMut},
    str::ParseBoolError,
};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    ContextSelectionSet, ID, InputType, InputValueError, InputValueResult, OutputType, Positioned,
    ServerResult, Value, parser::types::Field, registry::Registry,
};

/// Cursor type
///
/// A custom scalar that serializes as a string.
/// <https://relay.dev/graphql/connections.htm#sec-Cursor>
pub trait CursorType: Sized {
    /// Error type for `decode_cursor`.
    type Error: Display;

    /// Decode cursor from string.
    fn decode_cursor(s: &str) -> Result<Self, Self::Error>;

    /// Encode cursor to string.
    fn encode_cursor(&self) -> String;
}

macro_rules! cursor_type_int_impl {
    ($($t:ty)*) => {$(
        impl CursorType for $t {
            type Error = ParseIntError;

            fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }

            fn encode_cursor(&self) -> String {
                self.to_string()
            }
        }
    )*}
}

cursor_type_int_impl! { isize i8 i16 i32 i64 i128 usize u8 u16 u32 u64 u128 }

impl CursorType for f32 {
    type Error = ParseFloatError;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

impl CursorType for f64 {
    type Error = ParseFloatError;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

impl CursorType for char {
    type Error = ParseCharError;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

impl CursorType for bool {
    type Error = ParseBoolError;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

impl CursorType for String {
    type Error = Infallible;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        Ok(s.to_string())
    }

    fn encode_cursor(&self) -> String {
        self.clone()
    }
}

impl CursorType for ID {
    type Error = Infallible;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        Ok(s.to_string().into())
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

#[cfg(feature = "chrono")]
impl CursorType for chrono::DateTime<chrono::Utc> {
    type Error = chrono::ParseError;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        Ok(chrono::DateTime::parse_from_rfc3339(s)?.with_timezone::<chrono::Utc>(&chrono::Utc {}))
    }

    fn encode_cursor(&self) -> String {
        self.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
    }
}

#[cfg(feature = "jiff")]
impl CursorType for jiff::Timestamp {
    type Error = jiff::Error;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

#[cfg(feature = "uuid")]
impl CursorType for uuid::Uuid {
    type Error = uuid::Error;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

/// A opaque cursor that encode/decode the value to base64
///
/// `OpaqueCursor<T>` implements both [`CursorType`] and [`InputType`], so it can
/// be used directly as a GraphQL input argument (instead of accepting
/// `Option<String>` and manually decoding via connection helpers). In the GraphQL
/// schema it is represented as a `String` cursor.
///
/// # Examples
///
/// ```rust
/// use gqlrs::*;
/// use gqlrs::connection::*;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Clone)]
/// struct MyCursor {
///     id: i32,
///     offset: usize,
/// }
///
/// struct Query;
///
/// #[Object]
/// impl Query {
///     async fn items(
///         &self,
///         after: Option<OpaqueCursor<MyCursor>>,
///         first: Option<i32>,
///     ) -> Result<Connection<OpaqueCursor<MyCursor>, String>> {
///         // `after` is already decoded from the base64 opaque cursor
///         let start = after.map(|c| c.offset).unwrap_or(0);
///         // ...
///         # todo!()
///     }
/// }
/// ```
pub struct OpaqueCursor<T>(pub T);

impl<T> Deref for OpaqueCursor<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for OpaqueCursor<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> CursorType for OpaqueCursor<T>
where
    T: Serialize + DeserializeOwned,
{
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        use base64::Engine;

        let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        Ok(Self(serde_json::from_slice(&data)?))
    }

    fn encode_cursor(&self) -> String {
        use base64::Engine;

        let value = serde_json::to_vec(&self.0).unwrap_or_default();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync> InputType for OpaqueCursor<T> {
    type RawValueType = T;

    fn type_name() -> Cow<'static, str> {
        <String as InputType>::type_name()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        <String as InputType>::create_type_info(registry)
    }

    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        let cursor = <String as InputType>::parse(value).map_err(InputValueError::propagate)?;
        CursorType::decode_cursor(&cursor)
            .map_err(|e| InputValueError::custom(format!("Invalid opaque cursor: {e}")))
    }

    fn to_value(&self) -> Value {
        Value::String(CursorType::encode_cursor(self))
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(&self.0)
    }
}

#[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
impl<T: Serialize + DeserializeOwned + Send + Sync> OutputType for OpaqueCursor<T> {
    fn type_name() -> Cow<'static, str> {
        <String as OutputType>::type_name()
    }

    fn create_type_info(registry: &mut Registry) -> String {
        <String as OutputType>::create_type_info(registry)
    }

    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        Ok(Value::String(CursorType::encode_cursor(self)))
    }
}
