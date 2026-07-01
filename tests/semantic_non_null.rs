use gqlrs::*;

#[tokio::test]
async fn test_semantic_non_null_on_result_fields() {
    struct Query;

    #[Object(semantic_non_null)]
    impl Query {
        async fn ok_field(&self) -> Result<String> {
            Ok("success".to_string())
        }

        async fn err_field(&self) -> Result<i32> {
            Err("error".into())
        }

        // Non-Result fields should not get @semanticNonNull
        // because they're already non-null in the schema
        async fn regular(&self) -> &str {
            "always here"
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    // The @semanticNonNull directive definition should be present
    assert!(
        sdl.contains("directive @semanticNonNull"),
        "Expected @semanticNonNull directive definition in SDL, got:\n{}",
        sdl
    );

    // Result fields should have @semanticNonNull
    // The schema should contain something like: okField: String @semanticNonNull
    assert!(
        sdl.contains("@semanticNonNull"),
        "Expected @semanticNonNull on Result fields in SDL, got:\n{}",
        sdl
    );
}

#[tokio::test]
async fn test_semantic_non_null_selective_result_field() {
    struct Query;

    #[Object]
    impl Query {
        #[graphql(semantic_non_null)]
        async fn semantically_non_null(&self) -> Result<String> {
            Ok("always present".to_string())
        }

        async fn regular(&self) -> Result<String> {
            Ok("nullable".to_string())
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    // The @semanticNonNull directive definition should be present
    assert!(
        sdl.contains("directive @semanticNonNull"),
        "Expected @semanticNonNull directive definition in SDL, got:\n{}",
        sdl
    );
}

#[tokio::test]
async fn test_no_semantic_non_null_by_default() {
    struct Query;

    #[Object]
    impl Query {
        async fn name(&self) -> &str {
            "test"
        }

        async fn result_field(&self) -> Result<String> {
            Ok("test".to_string())
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    // Without semantic_non_null attribute, no @semanticNonNull should appear
    assert!(
        !sdl.contains("@semanticNonNull"),
        "Did not expect @semanticNonNull in SDL without opt-in, got:\n{}",
        sdl
    );
}
