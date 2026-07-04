use gqlrs::*;

struct DenyGuard;

#[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
impl Guard for DenyGuard {
    async fn check(&self, _ctx: &Context<'_>) -> Result<()> {
        Err("Forbidden".into())
    }
}

#[tokio::test]
async fn test_simple_object_optional_field() {
    /// A user with an optional name field.
    #[derive(SimpleObject)]
    struct User {
        id: i32,
        /// The name is optional in the schema but always present in Rust.
        #[graphql(optional)]
        name: String,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn user(&self) -> User {
            User {
                id: 1,
                name: "Alice".to_string(),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Verify that the schema declares `name` as nullable (String, not String!)
    let sdl = schema.sdl();
    assert!(
        sdl.contains("name: String\n") || sdl.contains("name: String "),
        "name field should be nullable (String, not String!):\n{}",
        sdl
    );
    assert!(
        !sdl.contains("name: String!"),
        "name field should NOT be non-null (String!):\n{}",
        sdl
    );

    // Verify that the id field is still non-null
    assert!(
        sdl.contains("id: Int!"),
        "id field should be non-null (Int!):\n{}",
        sdl
    );

    // Verify that we can query the field and get a value
    let query = r#"{ user { id name } }"#;
    let result = schema.execute(query).await.into_result().unwrap().data;
    assert_eq!(
        result,
        value!({
            "user": {
                "id": 1,
                "name": "Alice",
            }
        })
    );
}

#[tokio::test]
async fn test_simple_object_optional_field_returns_null() {
    /// A product where the description might not be available.
    #[derive(SimpleObject)]
    struct Product {
        id: i32,
        #[graphql(optional)]
        description: Option<String>,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn product(&self) -> Product {
            Product {
                id: 1,
                description: None,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Verify the field is nullable in schema
    let sdl = schema.sdl();
    assert!(
        !sdl.contains("description: String!"),
        "description field should be nullable:\n{}",
        sdl
    );

    // Query and verify null is returned properly
    let query = r#"{ product { id description } }"#;
    let result = schema.execute(query).await.into_result().unwrap().data;
    assert_eq!(
        result,
        value!({
            "product": {
                "id": 1,
                "description": null,
            }
        })
    );
}

#[tokio::test]
async fn test_simple_object_optional_field_with_value() {
    /// Same product test but with a value present.
    #[derive(SimpleObject)]
    struct Product {
        id: i32,
        #[graphql(optional)]
        description: Option<String>,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn product(&self) -> Product {
            Product {
                id: 1,
                description: Some("A great product".to_string()),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ product { id description } }"#;
    let result = schema.execute(query).await.into_result().unwrap().data;
    assert_eq!(
        result,
        value!({
            "product": {
                "id": 1,
                "description": "A great product",
            }
        })
    );
}

#[tokio::test]
async fn test_simple_object_optional_guard_failure_returns_null() {
    #[derive(SimpleObject)]
    struct Query {
        #[graphql(optional, guard = "DenyGuard")]
        value: i32,
        other: i32,
    }

    let schema = Schema::new(
        Query {
            value: 100,
            other: 1,
        },
        EmptyMutation,
        EmptySubscription,
    );
    let response = schema.execute("{ value other }").await;

    assert_eq!(response.data, value!({ "value": null, "other": 1 }));
    assert_eq!(
        response.errors,
        vec![ServerError {
            message: "Forbidden".to_string(),
            source: None,
            locations: vec![Pos { line: 1, column: 3 }],
            path: vec![PathSegment::Field("value".to_owned())],
            extensions: None,
        }]
    );
}
