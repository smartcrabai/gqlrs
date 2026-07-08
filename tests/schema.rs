use ::http::HeaderValue;
use gqlrs::*;

#[tokio::test]
pub async fn test_schema_default() {
    #[derive(Default)]
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> i32 {
            10
        }
    }

    type MySchema = Schema<Query, EmptyMutation, EmptySubscription>;

    let _schema = MySchema::default();
}

#[tokio::test]
pub async fn test_http_headers() {
    #[derive(Default)]
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self, ctx: &Context<'_>) -> i32 {
            ctx.insert_http_header("A", "1");
            10
        }

        async fn err(&self, ctx: &Context<'_>) -> FieldResult<i32> {
            ctx.insert_http_header("A", "1");
            Err("error".into())
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let resp = schema.execute("{ value }").await;
    assert_eq!(
        resp.http_headers.get("A"),
        Some(&HeaderValue::from_static("1"))
    );

    let resp = schema.execute("{ err }").await;
    assert_eq!(
        resp.http_headers.get("A"),
        Some(&HeaderValue::from_static("1"))
    );
}

#[tokio::test]
pub async fn test_complexity_with_const_expression() {
    const VALUE: usize = 5;

    struct Query;

    #[Object]
    impl Query {
        #[graphql(complexity = "VALUE")]
        async fn items(&self) -> Vec<i32> {
            vec![1, 2, 3]
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let resp = schema.execute("{ items }").await;
    assert_eq!(resp.data, value!({ "items": [1, 2, 3] }));
}

#[tokio::test]
pub async fn test_complexity_with_const_and_child() {
    const MULTIPLIER: usize = 3;

    struct Query;

    #[Object]
    impl Query {
        #[graphql(complexity = "MULTIPLIER * child_complexity")]
        async fn items(&self) -> Vec<i32> {
            vec![1, 2, 3]
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let resp = schema.execute("{ items }").await;
    assert_eq!(resp.data, value!({ "items": [1, 2, 3] }));
}
