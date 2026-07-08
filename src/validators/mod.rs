mod chars_max_length;
mod chars_min_length;
mod max_items;
mod max_length;
mod maximum;
mod min_items;
mod min_length;
mod minimum;
mod multiple_of;
mod regex;
#[cfg(feature = "validator")]
mod validator_adapter;

pub use chars_max_length::chars_max_length;
pub use chars_min_length::chars_min_length;
pub use max_items::max_items;
pub use max_length::max_length;
pub use maximum::maximum;
pub use min_items::min_items;
pub use min_length::min_length;
pub use minimum::minimum;
pub use multiple_of::multiple_of;

pub use self::regex::regex;
#[cfg(feature = "validator")]
#[cfg_attr(docsrs, doc(cfg(feature = "validator")))]
pub use self::validator_adapter::{ValidatorAdapter, ValidatorExt};
use crate::{Context, InputType, InputValueError};

/// Represents a custom input value validator.
pub trait CustomValidator<T: InputType> {
    /// Check the value is valid.
    fn check(&self, value: &T) -> Result<(), InputValueError<T>>;
}

impl<T, F, E> CustomValidator<T> for F
where
    T: InputType,
    E: Into<InputValueError<T>>,
    F: Fn(&T) -> Result<(), E>,
{
    #[inline]
    fn check(&self, value: &T) -> Result<(), InputValueError<T>> {
        (self)(value).map_err(Into::into)
    }
}

/// Represents a custom input value validator that has access to the request
/// context.
///
/// This allows validators to access data stored in the context, such as
/// database connections or other request-scoped resources, enabling
/// context-dependent validation like checking uniqueness against a database.
///
/// # Example
///
/// ```ignore
/// struct UniqueNameValidator;
///
/// impl CustomValidatorWithContext<String> for UniqueNameValidator {
///     fn check(&self, value: &String, ctx: &Context<'_>) -> Result<(), InputValueError<String>> {
///         let db = ctx.data::<DatabasePool>()?;
///         // check uniqueness against database
///         Ok(())
///     }
/// }
/// ```
pub trait CustomValidatorWithContext<T: InputType> {
    /// Check the value is valid, with access to the request context.
    fn check(&self, value: &T, ctx: &Context<'_>) -> Result<(), InputValueError<T>>;
}

impl<T, F, E> CustomValidatorWithContext<T> for F
where
    T: InputType,
    F: for<'a, 'b, 'c> Fn(&'a T, &'b Context<'c>) -> Result<(), E>,
    E: Into<InputValueError<T>>,
{
    #[inline]
    fn check(&self, value: &T, ctx: &Context<'_>) -> Result<(), InputValueError<T>> {
        (self)(value, ctx).map_err(Into::into)
    }
}
