use gqlrs::*;

#[tokio::test]
pub async fn test_nullable_result_field_on_object() {
    struct Query;

    #[Object]
    impl Query {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Err("something went wrong".into())
        }

        async fn other(&self) -> i32 {
            42
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // The nullable field should return null and add the error
    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "something went wrong");
    assert_eq!(
        response.errors[0].path,
        vec![PathSegment::Field("value".to_owned())]
    );
}

#[tokio::test]
pub async fn test_nullable_result_field_on_object_ok() {
    struct Query;

    #[Object]
    impl Query {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Ok(42)
        }

        async fn other(&self) -> i32 {
            100
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // When the Result is Ok, the value should be returned normally
    assert_eq!(response.data, value!({ "value": 42, "other": 100 }));
    assert!(response.errors.is_empty());
}

#[tokio::test]
pub async fn test_nullable_result_field_on_complex_object() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct Query {
        other: i32,
    }

    #[ComplexObject]
    impl Query {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Err("something went wrong".into())
        }
    }

    let schema = Schema::new(Query { other: 42 }, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // The nullable field should return null and add the error
    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "something went wrong");
}

#[tokio::test]
pub async fn test_nullable_result_field_on_complex_object_ok() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct Query {
        other: i32,
    }

    #[ComplexObject]
    impl Query {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Ok(42)
        }
    }

    let schema = Schema::new(Query { other: 100 }, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // When the Result is Ok, the value should be returned normally
    assert_eq!(response.data, value!({ "value": 42, "other": 100 }));
    assert!(response.errors.is_empty());
}

#[tokio::test]
pub async fn test_non_nullable_result_field_still_records_error() {
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> Result<i32> {
            Err("something went wrong".into())
        }

        async fn other(&self) -> i32 {
            42
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // Without nullable attribute, Result fields are still nullable and the
    // error is recorded at the field while the rest of the response succeeds.
    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "something went wrong");
    assert_eq!(
        response.errors[0].path,
        vec![PathSegment::Field("value".to_owned())]
    );
}

#[tokio::test]
pub async fn test_nullable_result_with_custom_error_type() {
    #[derive(Debug, Clone)]
    struct CustomError(String);

    impl From<CustomError> for Error {
        fn from(err: CustomError) -> Self {
            Error::new(err.0)
        }
    }

    struct Query;

    #[Object]
    impl Query {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32, CustomError> {
            Err(CustomError("custom error".to_string()))
        }

        async fn other(&self) -> i32 {
            42
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    // The nullable field should return null and add the error
    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "custom error");
}

#[tokio::test]
pub async fn test_nullable_result_with_guard() {
    #[derive(Eq, PartialEq, Copy, Clone)]
    enum Role {
        Admin,
        Guest,
    }

    pub struct RoleGuard {
        role: Role,
    }

    impl RoleGuard {
        fn new(role: Role) -> Self {
            Self { role }
        }
    }

    #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
    impl Guard for RoleGuard {
        async fn check(&self, ctx: &Context<'_>) -> Result<()> {
            if ctx.data_opt::<Role>() == Some(&self.role) {
                Ok(())
            } else {
                Err("Forbidden".into())
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        #[graphql(nullable, guard = "RoleGuard::new(Role::Admin)")]
        async fn value(&self) -> Result<i32> {
            Ok(42)
        }

        async fn other(&self) -> i32 {
            100
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // With admin role, the value should be returned
    let response = schema
        .execute(Request::new("{ value other }").data(Role::Admin))
        .await;
    assert_eq!(response.data, value!({ "value": 42, "other": 100 }));
    assert!(response.errors.is_empty());

    // With guest role, the guard should fail and return null (because nullable)
    let response = schema
        .execute(Request::new("{ value other }").data(Role::Guest))
        .await;
    assert_eq!(response.data, value!({ "value": null, "other": 100 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "Forbidden");
}

#[tokio::test]
pub async fn test_sync_nullable_result_field_on_object() {
    struct Query;

    #[Object]
    impl Query {
        #[graphql(nullable)]
        fn value(&self) -> Result<i32> {
            Err("something went wrong".into())
        }

        fn other(&self) -> i32 {
            42
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "something went wrong");
}

#[tokio::test]
pub async fn test_sync_nullable_result_field_on_complex_object() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct Query {
        other: i32,
    }

    #[ComplexObject]
    impl Query {
        #[graphql(nullable)]
        fn value(&self) -> Result<i32> {
            Err("something went wrong".into())
        }
    }

    let schema = Schema::new(Query { other: 42 }, EmptyMutation, EmptySubscription);
    let response = schema.execute(Request::new("{ value other }")).await;

    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "something went wrong");
}

#[tokio::test]
pub async fn test_nullable_result_field_on_simple_object() {
    #[derive(Debug, Clone)]
    struct CustomError(String);

    impl From<CustomError> for Error {
        fn from(err: CustomError) -> Self {
            Error::new(err.0)
        }
    }

    #[derive(SimpleObject)]
    struct Query {
        #[graphql(nullable)]
        value: Result<i32, CustomError>,
        other: i32,
    }

    let schema = Schema::new(
        Query {
            value: Err(CustomError("custom error".to_string())),
            other: 42,
        },
        EmptyMutation,
        EmptySubscription,
    );
    let response = schema.execute(Request::new("{ value other }")).await;

    assert_eq!(response.data, value!({ "value": null, "other": 42 }));
    assert!(!response.errors.is_empty());
    assert_eq!(response.errors[0].message, "custom error");
}

#[test]
pub fn test_nullable_result_fields_are_nullable_in_sdl() {
    fn assert_field(sdl: &str, expected: &str) {
        assert!(
            sdl.lines().any(|line| line.trim() == expected),
            "expected `{expected}` in SDL:\n{sdl}"
        );
    }

    struct ObjectQuery;

    #[Object]
    impl ObjectQuery {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Ok(42)
        }

        async fn other(&self) -> i32 {
            100
        }
    }

    let sdl = Schema::new(ObjectQuery, EmptyMutation, EmptySubscription).sdl();
    assert_field(&sdl, "value: Int");
    assert_field(&sdl, "other: Int!");

    #[derive(SimpleObject)]
    struct SimpleQuery {
        #[graphql(nullable)]
        value: Result<i32>,
        other: i32,
    }

    let sdl = Schema::new(
        SimpleQuery {
            value: Ok(42),
            other: 100,
        },
        EmptyMutation,
        EmptySubscription,
    )
    .sdl();
    assert_field(&sdl, "value: Int");
    assert_field(&sdl, "other: Int!");

    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct ComplexQuery {
        other: i32,
    }

    #[ComplexObject]
    impl ComplexQuery {
        #[graphql(nullable)]
        async fn value(&self) -> Result<i32> {
            Ok(42)
        }
    }

    let sdl = Schema::new(
        ComplexQuery { other: 100 },
        EmptyMutation,
        EmptySubscription,
    )
    .sdl();
    assert_field(&sdl, "value: Int");
    assert_field(&sdl, "other: Int!");
}
