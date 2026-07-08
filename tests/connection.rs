use gqlrs::{connection::*, *};
use serde::{Deserialize, Serialize};

#[tokio::test]
pub async fn test_connection_additional_fields() {
    struct Query;

    #[derive(SimpleObject)]
    struct ConnectionFields {
        total_count: i32,
    }

    #[derive(SimpleObject)]
    struct Diff {
        diff: i32,
    }

    #[Object]
    impl Query {
        async fn numbers(
            &self,
            after: Option<String>,
            before: Option<String>,
            first: Option<i32>,
            last: Option<i32>,
        ) -> Result<Connection<usize, i32, ConnectionFields, Diff>> {
            connection::query(
                after,
                before,
                first,
                last,
                |after, before, first, last| async move {
                    let mut start = after.map(|after| after + 1).unwrap_or(0);
                    let mut end = before.unwrap_or(10000);
                    if let Some(first) = first {
                        end = (start + first).min(end);
                    }
                    if let Some(last) = last {
                        start = if last > end - start { end } else { end - last };
                    }
                    let mut connection = Connection::with_additional_fields(
                        start > 0,
                        end < 10000,
                        ConnectionFields { total_count: 10000 },
                    );
                    connection.edges.extend((start..end).map(|n| {
                        Edge::with_additional_fields(
                            n,
                            n as i32,
                            Diff {
                                diff: (10000 - n) as i32,
                            },
                        )
                    }));
                    Ok::<_, Error>(connection)
                },
            )
            .await
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    assert_eq!(
        schema
            .execute(
                "{ numbers(first: 2) { __typename totalCount edges { __typename node diff } } }"
            )
            .await
            .data,
        value!({
            "numbers": {
                "__typename": "IntConnection",
                "totalCount": 10000,
                "edges": [
                    {"__typename": "IntEdge", "node": 0, "diff": 10000},
                    {"__typename": "IntEdge", "node": 1, "diff": 9999},
                ]
            },
        })
    );

    assert_eq!(
        schema
            .execute("{ numbers(last: 2) { edges { node diff } } }")
            .await
            .data,
        value!({
            "numbers": {
                "edges": [
                    {"node": 9998, "diff": 2},
                    {"node": 9999, "diff": 1},
                ]
            },
        })
    );
}

#[tokio::test]
pub async fn test_connection_nodes() {
    struct Query;

    #[Object]
    impl Query {
        async fn numbers(
            &self,
            after: Option<String>,
            before: Option<String>,
            first: Option<i32>,
            last: Option<i32>,
        ) -> Result<Connection<usize, i32>> {
            connection::query(
                after,
                before,
                first,
                last,
                |after, before, first, last| async move {
                    let mut start = after.map(|after| after + 1).unwrap_or(0);
                    let mut end = before.unwrap_or(10000);
                    if let Some(first) = first {
                        end = (start + first).min(end);
                    }
                    if let Some(last) = last {
                        start = if last > end - start { end } else { end - last };
                    }
                    let mut connection = Connection::new(start > 0, end < 10000);
                    connection
                        .edges
                        .extend((start..end).map(|n| Edge::new(n, n as i32)));
                    Ok::<_, Error>(connection)
                },
            )
            .await
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    assert_eq!(
        schema
            .execute("{ numbers(first: 2) { __typename edges { __typename node } nodes } }")
            .await
            .data,
        value!({
            "numbers": {
                "__typename": "IntConnection",
                "edges": [
                    {"__typename": "IntEdge", "node": 0},
                    {"__typename": "IntEdge", "node": 1},
                ],
                "nodes": [
                    0,
                    1,
                ],
            },
        })
    );

    assert_eq!(
        schema.execute("{ numbers(last: 2) { nodes } }").await.data,
        value!({
            "numbers": {
                "nodes": [
                    9998,
                    9999,
                ],
            },
        })
    );
}

#[tokio::test]
pub async fn test_opaque_cursor_inputs_use_string_scalar() {
    #[derive(Serialize, Deserialize)]
    struct UserCursor {
        id: i32,
    }

    #[derive(Serialize, Deserialize)]
    struct PostCursor {
        id: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn user_id(&self, after: Option<OpaqueCursor<UserCursor>>) -> i32 {
            after.map(|cursor| cursor.id).unwrap_or_default()
        }

        async fn post_id(&self, after: Option<OpaqueCursor<PostCursor>>) -> i32 {
            after.map(|cursor| cursor.id).unwrap_or_default()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let user_cursor = OpaqueCursor(UserCursor { id: 10 }).encode_cursor();
    let post_cursor = OpaqueCursor(PostCursor { id: 20 }).encode_cursor();
    let query = format!(
        r#"{{ userId(after: "{}") postId(after: "{}") }}"#,
        user_cursor, post_cursor
    );

    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "userId": 10,
            "postId": 20,
        })
    );
}

#[tokio::test]
pub async fn test_opaque_cursor_as_input() {
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    struct MyCursor {
        offset: usize,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn numbers(
            &self,
            after: Option<OpaqueCursor<MyCursor>>,
            first: Option<i32>,
        ) -> Result<Connection<OpaqueCursor<MyCursor>, i32>> {
            let start = after.map(|c| c.offset + 1).unwrap_or(0);
            let first = first.unwrap_or(3) as usize;
            let end = (start + first).min(10000);
            let mut connection = Connection::new(start > 0, end < 10000);
            connection.edges.extend(
                (start..end).map(|n| Edge::new(OpaqueCursor(MyCursor { offset: n }), n as i32)),
            );
            Ok(connection)
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let encoded = OpaqueCursor(MyCursor { offset: 1 }).encode_cursor();

    // First page: request two items and expose the opaque end cursor.
    let res = schema
        .execute(r#"{ numbers(first: 2) { edges { node } pageInfo { endCursor hasNextPage } } }"#)
        .await;
    assert!(
        res.errors.is_empty(),
        "First page had errors: {:?}",
        res.errors
    );
    assert_eq!(
        res.data,
        value!({
            "numbers": {
                "edges": [
                    { "node": 0 },
                    { "node": 1 },
                ],
                "pageInfo": {
                    "endCursor": encoded.clone(),
                    "hasNextPage": true,
                },
            }
        })
    );

    // Second page: pass the opaque cursor directly as an input argument.
    let query = format!(
        r#"{{ numbers(after: "{}", first: 2) {{ edges {{ node }} }} }}"#,
        encoded
    );
    let res2 = schema.execute(&query).await;
    assert!(
        res2.errors.is_empty(),
        "Second page had errors: {:?}",
        res2.errors
    );

    assert_eq!(
        res2.data,
        value!({
            "numbers": {
                "edges": [
                    { "node": 2 },
                    { "node": 3 },
                ]
            }
        })
    );
}

#[tokio::test]
pub async fn test_connection_nodes_disabled() {
    struct Query;

    #[Object]
    impl Query {
        async fn numbers(
            &self,
            after: Option<String>,
            before: Option<String>,
            first: Option<i32>,
            last: Option<i32>,
        ) -> Result<
            Connection<
                usize,
                i32,
                EmptyFields,
                EmptyFields,
                DefaultConnectionName,
                DefaultEdgeName,
                DisableNodesField,
            >,
        > {
            connection::query(
                after,
                before,
                first,
                last,
                |after, before, first, last| async move {
                    let mut start = after.map(|after| after + 1).unwrap_or(0);
                    let mut end = before.unwrap_or(10000);
                    if let Some(first) = first {
                        end = (start + first).min(end);
                    }
                    if let Some(last) = last {
                        start = if last > end - start { end } else { end - last };
                    }
                    let mut connection = Connection::new(start > 0, end < 10000);
                    connection
                        .edges
                        .extend((start..end).map(|n| Edge::new(n, n as i32)));
                    Ok::<_, Error>(connection)
                },
            )
            .await
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let r = schema.execute("{ numbers(last: 2) { nodes } }").await;

    assert_eq!(
        r.errors[0].message,
        "Unknown field \"nodes\" on type \"IntConnection\"."
    );
}
