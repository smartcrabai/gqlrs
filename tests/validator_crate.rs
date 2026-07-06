#![cfg(feature = "validator")]

use gqlrs::*;
use validator::Validate;

#[derive(InputObject, Validate)]
struct ValidatedInput {
    #[validate(length(min = 1, max = 100))]
    name: String,
    #[validate(range(min = 0, max = 150))]
    age: i32,
}

#[derive(InputObject, Validate)]
struct EmailInput {
    #[validate(email)]
    email: String,
}

fn validation_error_fields(value: &Value) -> Vec<String> {
    match value {
        Value::List(errors) => errors
            .iter()
            .map(|error| match error {
                Value::Object(details) => match details.get("field").unwrap() {
                    Value::String(field) => field.clone(),
                    value => panic!("expected field to be a string, got {value:?}"),
                },
                value => panic!("expected validation error details to be objects, got {value:?}"),
            })
            .collect(),
        value => panic!("expected validation_errors to be a list, got {value:?}"),
    }
}

#[tokio::test]
async fn test_validator_adapter_on_input_object() {
    struct Query;

    #[Object]
    impl Query {
        async fn create_user(
            &self,
            #[graphql(validator(
                custom = "gqlrs::validators::ValidatorAdapter::<ValidatedInput>::new()"
            ))]
            input: ValidatedInput,
        ) -> String {
            input.name.clone()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Valid input
    let res = schema
        .execute(r#"{ createUser(input: { name: "Alice", age: 30 }) }"#)
        .await;
    assert_eq!(
        res.into_result().unwrap().data,
        value!({ "createUser": "Alice" })
    );

    // Invalid: name too short (empty)
    let res = schema
        .execute(r#"{ createUser(input: { name: "", age: 30 }) }"#)
        .await;
    let errors = res.into_result().unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors[0].message.contains("name") || errors[0].message.contains("length"));

    // Invalid: age out of range
    let res = schema
        .execute(r#"{ createUser(input: { name: "Alice", age: 200 }) }"#)
        .await;
    let errors = res.into_result().unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors[0].message.contains("age") || errors[0].message.contains("range"));
}

#[tokio::test]
async fn test_validator_adapter_on_field_arg() {
    struct Query;

    #[Object]
    impl Query {
        async fn validate_email(
            &self,
            #[graphql(validator(
                custom = "gqlrs::validators::ValidatorAdapter::<EmailInput>::new()"
            ))]
            input: EmailInput,
        ) -> String {
            input.email.clone()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Valid email
    let res = schema
        .execute(r#"{ validateEmail(input: { email: "user@example.com" }) }"#)
        .await;
    assert_eq!(
        res.into_result().unwrap().data,
        value!({ "validateEmail": "user@example.com" })
    );

    // Invalid email
    let res = schema
        .execute(r#"{ validateEmail(input: { email: "not-an-email" }) }"#)
        .await;
    let errors = res.into_result().unwrap_err();
    assert!(!errors.is_empty());
    assert!(
        errors[0].message.contains("email"),
        "Expected error about email, got: {}",
        errors[0].message
    );
}

#[tokio::test]
async fn test_validator_ext_trait() {
    // Test that ValidatorExt properly converts errors
    let mut errors = validator::ValidationErrors::new();
    errors.add(
        "test_field",
        validator::ValidationError::new("test_code").with_message("test message".into()),
    );

    let input_err: InputValueError<String> = errors.into_input_value_error();
    let server_err = input_err.into_server_error(Pos { line: 1, column: 1 });
    assert!(server_err.message.contains("test"));

    let ext = server_err.extensions.as_ref().unwrap();
    assert_eq!(
        validation_error_fields(ext.get("validation_errors").unwrap()),
        vec!["test_field".to_string()]
    );
}

#[tokio::test]
async fn test_validator_adapter_with_optional_field() {
    #[derive(InputObject, Validate)]
    struct OptionalInput {
        #[validate(length(min = 1))]
        required_name: String,
        #[validate(range(min = 0, max = 100))]
        optional_score: Option<i32>,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn process(
            &self,
            #[graphql(validator(
                custom = "gqlrs::validators::ValidatorAdapter::<OptionalInput>::new()"
            ))]
            input: OptionalInput,
        ) -> i32 {
            input.optional_score.unwrap_or(0)
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Valid with optional field present
    let res = schema
        .execute(r#"{ process(input: { requiredName: "test", optionalScore: 50 }) }"#)
        .await;
    assert_eq!(res.into_result().unwrap().data, value!({ "process": 50 }));

    // Valid with optional field absent
    let res = schema
        .execute(r#"{ process(input: { requiredName: "test" }) }"#)
        .await;
    assert_eq!(res.into_result().unwrap().data, value!({ "process": 0 }));
}

#[tokio::test]
async fn test_validator_adapter_error_extensions() {
    struct Query;

    #[Object]
    impl Query {
        async fn validate_email(
            &self,
            #[graphql(validator(
                custom = "gqlrs::validators::ValidatorAdapter::<EmailInput>::new()"
            ))]
            input: EmailInput,
        ) -> String {
            input.email.clone()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Invalid email should have extensions with field info
    let res = schema
        .execute(r#"{ validateEmail(input: { email: "bad" }) }"#)
        .await;
    let errors = res.into_result().unwrap_err();
    assert!(!errors.is_empty());

    // Check that extensions contain field information
    let err = &errors[0];
    assert!(err.extensions.is_some(), "Expected error extensions");
    let ext = err.extensions.as_ref().unwrap();
    assert_eq!(
        validation_error_fields(ext.get("validation_errors").unwrap()),
        vec!["email".to_string()]
    );
}

#[tokio::test]
async fn test_validator_adapter_preserves_multiple_error_extensions() {
    struct Query;

    #[Object]
    impl Query {
        async fn create_user(
            &self,
            #[graphql(validator(
                custom = "gqlrs::validators::ValidatorAdapter::<ValidatedInput>::new()"
            ))]
            input: ValidatedInput,
        ) -> i32 {
            input.age
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ createUser(input: { name: "", age: 200 }) }"#)
        .await;
    let errors = res.into_result().unwrap_err();
    assert!(!errors.is_empty());

    let ext = errors[0].extensions.as_ref().unwrap();
    let mut fields = validation_error_fields(ext.get("validation_errors").unwrap());
    fields.sort();

    assert_eq!(fields, vec!["age".to_string(), "name".to_string()]);
}
