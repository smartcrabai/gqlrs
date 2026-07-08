use std::marker::PhantomData;

use indexmap::IndexMap;

use crate::{InputType, InputValueError, Name, Value};

/// Extension trait for converting [`validator::ValidationErrors`] into
/// [`InputValueError`].
///
/// This allows seamless integration between the `validator` crate and
/// async-graphql's validation system.
pub trait ValidatorExt<T: InputType> {
    /// Convert validation errors into an `InputValueError`.
    fn into_input_value_error(self) -> InputValueError<T>;
}

impl<T: InputType> ValidatorExt<T> for validator::ValidationErrors {
    fn into_input_value_error(self) -> InputValueError<T> {
        validation_errors_into_input_value_error(self)
    }
}

/// Adapter that bridges the [`validator::Validate`] trait to
/// async-graphql's [`CustomValidator`](crate::CustomValidator).
///
/// Use this to run `validator`-crate validations on input objects without
/// duplicating validation logic.
///
/// # Example
///
/// ```rust,no_run
/// use gqlrs::*;
/// use validator::Validate;
///
/// #[derive(InputObject, Validate)]
/// struct CreateUserInput {
///     #[validate(length(min = 1, max = 100))]
///     name: String,
///     #[validate(email)]
///     email: String,
/// }
///
/// struct Query;
///
/// #[Object]
/// impl Query {
///     async fn create_user(
///         &self,
///         #[graphql(validator(
///             custom = "gqlrs::validators::ValidatorAdapter::<CreateUserInput>::new()"
///         ))]
///         input: CreateUserInput,
///     ) -> bool {
///         true
///     }
/// }
/// ```
pub struct ValidatorAdapter<T: InputType + validator::Validate> {
    _phantom: PhantomData<T>,
}

impl<T: InputType + validator::Validate> ValidatorAdapter<T> {
    /// Create a new `ValidatorAdapter` for the given type.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: InputType + validator::Validate> Default for ValidatorAdapter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: InputType + validator::Validate> crate::CustomValidator<T> for ValidatorAdapter<T> {
    fn check(&self, value: &T) -> Result<(), InputValueError<T>> {
        value
            .validate()
            .map_err(ValidatorExt::into_input_value_error)
    }
}

fn validation_errors_into_input_value_error<T: InputType>(
    errors: validator::ValidationErrors,
) -> InputValueError<T> {
    let details = validation_error_details(&errors);
    let mut err = InputValueError::custom(errors.to_string());

    if !details.is_empty() {
        err = err.with_extension("validation_errors", details);
    }

    err
}

fn validation_error_details(errors: &validator::ValidationErrors) -> Vec<Value> {
    errors
        .field_errors()
        .iter()
        .flat_map(|(field, field_errors)| {
            field_errors.iter().map(move |field_error| {
                let mut details = IndexMap::new();
                details.insert(Name::new("field"), Value::from(field.to_string()));
                details.insert(Name::new("code"), Value::from(field_error.code.to_string()));

                if let Some(message) = &field_error.message {
                    details.insert(Name::new("message"), Value::from(message.to_string()));
                }

                Value::Object(details)
            })
        })
        .collect()
}
