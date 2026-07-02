use gqlrs::*;

#[tokio::test]
pub async fn test_error_formatter_adds_extensions() {
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> Result<i32> {
            Err(Error::new("test error"))
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .error_formatter(|mut error| {
            error
                .extensions
                .get_or_insert_with(ErrorExtensionValues::default)
                .set("formatted", true);
            error
        })
        .finish();

    let resp = schema.execute("{ value }").await;
    assert!(resp.is_err());
    assert_eq!(resp.errors.len(), 1);

    let extensions = resp.errors[0].extensions.as_ref().unwrap();
    assert_eq!(extensions.get("formatted"), Some(&Value::Boolean(true)));
}

#[tokio::test]
pub async fn test_error_formatter_rewrites_message() {
    struct Query;

    #[Object]
    impl Query {
        async fn fail(&self) -> Result<i32> {
            Err(Error::new("internal secret details"))
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .error_formatter(|mut error| {
            error.message = "An internal error occurred".to_string();
            error
        })
        .finish();

    let resp = schema.execute("{ fail }").await;
    assert!(resp.is_err());
    assert_eq!(resp.errors[0].message, "An internal error occurred");
}

#[tokio::test]
pub async fn test_error_formatter_applied_to_validation_errors() {
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> i32 {
            100
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .error_formatter(|mut error| {
            error
                .extensions
                .get_or_insert_with(ErrorExtensionValues::default)
                .set("source", "formatter");
            error
        })
        .finish();

    // Query a non-existent field to trigger a validation error
    let resp = schema.execute("{ nonexistent }").await;
    assert!(resp.is_err());
    assert!(!resp.errors.is_empty());

    let extensions = resp.errors[0].extensions.as_ref().unwrap();
    assert_eq!(
        extensions.get("source"),
        Some(&Value::String("formatter".to_string()))
    );
}

#[tokio::test]
pub async fn test_no_error_formatter() {
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> Result<i32> {
            Err(Error::new("test error"))
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let resp = schema.execute("{ value }").await;
    assert!(resp.is_err());
    assert_eq!(resp.errors.len(), 1);
    assert_eq!(resp.errors[0].message, "test error");
    assert!(resp.errors[0].extensions.is_none());
}

#[cfg(feature = "dynamic-schema")]
#[tokio::test]
pub async fn test_dynamic_schema_error_formatter() {
    use gqlrs::dynamic::*;

    let query =
        Object::new("Query").field(Field::new("value", TypeRef::named_nn(TypeRef::INT), |_| {
            FieldFuture::new(async { Ok(Some(Value::from(100))) })
        }));

    let schema = Schema::build("Query", None, None)
        .register(query)
        .error_formatter(|mut error| {
            error
                .extensions
                .get_or_insert_with(ErrorExtensionValues::default)
                .set("dynamic", true);
            error
        })
        .finish()
        .unwrap();

    // Query a non-existent field to trigger a validation error
    let resp = schema.execute("{ nonexistent }").await;
    assert!(resp.is_err());

    let extensions = resp.errors[0].extensions.as_ref().unwrap();
    assert_eq!(extensions.get("dynamic"), Some(&Value::Boolean(true)));
}
