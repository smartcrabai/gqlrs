use gqlrs::*;

#[tokio::test]
async fn test_semantic_non_null_simple_object() {
    #[derive(SimpleObject)]
    struct MyObj {
        #[graphql(semantic_non_null)]
        name: String,
        age: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj {
                name: "test".to_string(),
                age: 30,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    assert!(sdl.contains("@semanticNonNull"));
    assert!(sdl.contains("name: String! @semanticNonNull"));
    assert!(sdl.contains("age: Int!"));
}

#[tokio::test]
async fn test_semantic_non_null_object_macro() {
    struct MyObj;

    #[Object]
    impl MyObj {
        #[graphql(semantic_non_null)]
        async fn name(&self) -> &str {
            "test"
        }

        async fn age(&self) -> i32 {
            30
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    assert!(sdl.contains("@semanticNonNull"));
    assert!(sdl.contains("name: String! @semanticNonNull"));
    assert!(sdl.contains("age: Int!"));
}

#[tokio::test]
async fn test_semantic_non_null_sdl_export() {
    #[derive(SimpleObject)]
    struct MyObj {
        #[graphql(semantic_non_null)]
        name: String,
        age: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj {
                name: "test".to_string(),
                age: 30,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl_with_options(SDLExportOptions::new().use_space_ident().indent_width(2));

    // Verify the directive is present in the SDL
    assert!(sdl.contains("directive @semanticNonNull"));
    assert!(sdl.contains("name: String! @semanticNonNull"));
    assert!(sdl.contains("age: Int!"));
}

#[tokio::test]
async fn test_semantic_non_null_query_execution() {
    #[derive(SimpleObject)]
    struct MyObj {
        #[graphql(semantic_non_null)]
        name: String,
        age: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj {
                name: "test".to_string(),
                age: 30,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"
        {
            obj {
                name
                age
            }
        }
    "#;

    let result = schema.execute(query).await;
    assert!(result.errors.is_empty());
    assert_eq!(
        result.data,
        value!({
            "obj": {
                "name": "test",
                "age": 30
            }
        })
    );
}

#[tokio::test]
async fn test_semantic_non_null_not_present_when_not_used() {
    #[derive(SimpleObject)]
    struct MyObj {
        name: String,
        age: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj {
                name: "test".to_string(),
                age: 30,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();

    // Verify the directive is NOT present when not used
    assert!(!sdl.contains("@semanticNonNull"));
}

#[tokio::test]
async fn test_semantic_non_null_directive_in_sdl() {
    #[derive(SimpleObject)]
    struct MyObj {
        #[graphql(semantic_non_null)]
        name: String,
        age: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj {
                name: "test".to_string(),
                age: 30,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let sdl = schema.sdl();
    println!("SDL: {}", sdl);

    // Verify the directive definition is present
    assert!(sdl.contains("@semanticNonNull"));
}
