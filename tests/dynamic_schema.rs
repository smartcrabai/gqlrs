#[cfg(feature = "dynamic-schema")]
mod tests {
    use gqlrs::{
        Value,
        dynamic::{
            Directive, DynamicConnection, DynamicEdge, Enum, EnumItem, Field, FieldFuture,
            FieldValue, InputObject, InputValue, Interface, InterfaceField, Object,
            ResolverContext, Scalar, Schema, SchemaError, TypeRef, Union,
        },
        value,
    };

    fn mock_resolver_fn(_ctx: ResolverContext) -> FieldFuture {
        FieldFuture::Value(None)
    }

    pub fn schema() -> Result<Schema, SchemaError> {
        let test_enum = Enum::new("TestEnum")
            .item(EnumItem::new("A"))
            .item(EnumItem::new("B").directive(Directive::new("default")))
            .item(EnumItem::new("C"))
            .directive(Directive::new("oneOf"));

        let interface = Interface::new("TestInterface")
            .field(
                InterfaceField::new("id", TypeRef::named_nn(TypeRef::STRING))
                    .directive(Directive::new("id")),
            )
            .field(InterfaceField::new(
                "name",
                TypeRef::named_nn(TypeRef::STRING),
            ))
            .directive(
                Directive::new("test")
                    .argument("a", Value::from(5))
                    .argument("b", Value::from(true))
                    .argument("c", Value::from("str")),
            );

        let output_type = Object::new("OutputType")
            .implement(interface.type_name())
            .field(
                Field::new("id", TypeRef::named_nn(TypeRef::STRING), mock_resolver_fn)
                    .directive(Directive::new("test")),
            )
            .field(Field::new(
                "name",
                TypeRef::named_nn(TypeRef::STRING),
                mock_resolver_fn,
            ))
            .field(Field::new(
                "body",
                TypeRef::named(TypeRef::STRING),
                mock_resolver_fn,
            ))
            .directive(Directive::new("type"));

        let output_type_2 = Object::new("OutputType2").field(Field::new(
            "a",
            TypeRef::named_nn_list_nn(TypeRef::INT),
            mock_resolver_fn,
        ));

        let union_type = Union::new("TestUnion")
            .possible_type(output_type.type_name())
            .possible_type(output_type_2.type_name())
            .directive(Directive::new("wrap"));

        let input_type = InputObject::new("InputType")
            .field(
                InputValue::new("a", TypeRef::named_nn(TypeRef::STRING))
                    .directive(Directive::new("input_a").argument("test", Value::from(5))),
            )
            .directive(Directive::new("a"))
            .directive(Directive::new("b"));

        let scalar = Scalar::new("TestScalar").directive(Directive::new("json"));

        let query = Object::new("Query")
            .field(
                Field::new(
                    "interface",
                    TypeRef::named_nn(interface.type_name()),
                    mock_resolver_fn,
                )
                .argument(
                    InputValue::new("x", TypeRef::named(test_enum.type_name()))
                        .directive(Directive::new("validate")),
                ),
            )
            .field(
                Field::new(
                    "output_type",
                    TypeRef::named(output_type.type_name()),
                    mock_resolver_fn,
                )
                .argument(InputValue::new(
                    "input",
                    TypeRef::named_nn(input_type.type_name()),
                )),
            )
            .field(
                Field::new(
                    "enum",
                    TypeRef::named(test_enum.type_name()),
                    mock_resolver_fn,
                )
                .argument(InputValue::new(
                    "input",
                    TypeRef::named_list_nn(test_enum.type_name()),
                ))
                .directive(Directive::new("pin")),
            )
            .field(Field::new(
                "union",
                TypeRef::named_nn(union_type.type_name()),
                mock_resolver_fn,
            ))
            .field(Field::new(
                "scalar",
                TypeRef::named(scalar.type_name()),
                mock_resolver_fn,
            ));

        Schema::build(query.type_name(), None, None)
            .register(test_enum)
            .register(interface)
            .register(input_type)
            .register(output_type)
            .register(output_type_2)
            .register(union_type)
            .register(scalar)
            .register(query)
            .finish()
    }

    #[test]
    fn test_schema_sdl() {
        let schema = schema().unwrap();
        let sdl = schema.sdl();

        let expected = include_str!("schemas/test_dynamic_schema.graphql");

        assert_eq!(sdl, expected);
    }

    #[tokio::test]
    async fn field_future_from_future_can_be_awaited() {
        let value = FieldFuture::new(async { Ok(Some(Value::from(42))) })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(value.try_to_value().unwrap(), &Value::from(42));
    }

    #[tokio::test]
    async fn field_future_from_value_can_be_awaited() {
        let value = FieldFuture::from_value(Some(Value::from(42)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(value.try_to_value().unwrap(), &Value::from(42));
    }

    #[tokio::test]
    async fn test_dynamic_connection() {
        // Define a simple node type
        let item_type = Object::new("Item").field(Field::new(
            "id",
            TypeRef::named_nn(TypeRef::STRING),
            |ctx| {
                FieldFuture::new(async move {
                    // The parent_value is the node value itself (stored as FieldValue::Value)
                    let node = ctx.parent_value.as_value().cloned().unwrap_or(Value::Null);
                    Ok(Some(node))
                })
            },
        ));

        // Build connection types
        let conn_builder = DynamicConnection::builder("Item")
            .node_type_name("Item")
            .edge_field("index", TypeRef::named_nn(TypeRef::INT))
            .connection_field("totalCount", TypeRef::named(TypeRef::INT));

        // Build query
        let query = Object::new("Query").field(
            Field::new(
                "items",
                TypeRef::named_nn(conn_builder.connection_type_name()),
                |ctx| {
                    FieldFuture::new(async move {
                        let first: Option<i32> = ctx
                            .args
                            .get("first")
                            .and_then(|v| v.i64().ok())
                            .map(|n| n as i32);
                        let limit = first.unwrap_or(3).clamp(0, 100) as usize;
                        let mut conn = DynamicConnection::new(false, limit < 100)
                            .extra_field("totalCount", Value::from(limit as i64));
                        for i in 0..limit {
                            let id = format!("item-{}", i);
                            conn = conn.edge(
                                DynamicEdge::new(format!("cursor-{}", i), Value::from(id))
                                    .extra_field("index", Value::from(i as i64)),
                            );
                        }
                        Ok(Some(FieldValue::owned_any(conn)))
                    })
                },
            )
            .argument(InputValue::new("first", TypeRef::named(TypeRef::INT))),
        );

        let schema = Schema::build("Query", None, None)
            .register(item_type)
            .register(conn_builder.page_info_object())
            .register(conn_builder.edge_object())
            .register(conn_builder.connection_object())
            .register(query)
            .finish()
            .unwrap();

        // Test basic query
        let result = schema
            .execute("{ items(first: 2) { edges { cursor node { id } index } nodes { id } totalCount } }")
            .await;

        let data = result.into_result().unwrap().data;
        assert_eq!(
            data,
            value!({
                "items": {
                    "edges": [
                        {"cursor": "cursor-0", "node": {"id": "item-0"}, "index": 0},
                        {"cursor": "cursor-1", "node": {"id": "item-1"}, "index": 1},
                    ],
                    "nodes": [
                        {"id": "item-0"},
                        {"id": "item-1"},
                    ],
                    "totalCount": 2
                }
            })
        );
    }

    #[tokio::test]
    async fn test_dynamic_connection_page_info() {
        // Build a connection with page info
        let conn_builder = DynamicConnection::builder("Node");

        let node_type = Object::new("Node").field(Field::new(
            "value",
            TypeRef::named_nn(TypeRef::INT),
            |ctx| {
                FieldFuture::new(async move {
                    // The parent_value is the node value itself (stored as FieldValue::Value)
                    let node = ctx.parent_value.as_value().cloned().unwrap_or(Value::Null);
                    Ok(Some(node))
                })
            },
        ));

        let query = Object::new("Query").field(Field::new(
            "nodes",
            TypeRef::named_nn(conn_builder.connection_type_name()),
            |_ctx| {
                FieldFuture::new(async move {
                    let mut conn = DynamicConnection::new(true, true);
                    conn.page_info.start_cursor = Some("override-start".to_string());
                    conn = conn
                        .edge(DynamicEdge::new("a", Value::from(1)))
                        .edge(DynamicEdge::new("b", Value::from(2)))
                        .edge(DynamicEdge::new("c", Value::from(3)));
                    Ok(Some(FieldValue::owned_any(conn)))
                })
            },
        ));

        let schema = Schema::build("Query", None, None)
            .register(node_type)
            .register(conn_builder.page_info_object())
            .register(conn_builder.edge_object())
            .register(conn_builder.connection_object())
            .register(query)
            .finish()
            .unwrap();

        let result = schema
            .execute(
                "{ nodes { pageInfo { hasPreviousPage hasNextPage startCursor endCursor } edges { cursor node { value } } } }",
            )
            .await;

        let data = result.into_result().unwrap().data;
        assert_eq!(
            data,
            value!({
                "nodes": {
                    "pageInfo": {
                        "hasPreviousPage": true,
                        "hasNextPage": true,
                        "startCursor": "override-start",
                        "endCursor": "c"
                    },
                    "edges": [
                        {"cursor": "a", "node": {"value": 1}},
                        {"cursor": "b", "node": {"value": 2}},
                        {"cursor": "c", "node": {"value": 3}},
                    ]
                }
            })
        );
    }

    #[tokio::test]
    async fn test_dynamic_connection_custom_names() {
        // Test custom type names
        let conn_builder = DynamicConnection::builder("User")
            .connection_name("UserList")
            .edge_name("UserEntry")
            .page_info_name("UserPageInfo")
            .node_type_name("User");

        assert_eq!(conn_builder.connection_type_name(), "UserList");
        assert_eq!(conn_builder.edge_type_name(), "UserEntry");
        assert_eq!(conn_builder.page_info_type_name(), "UserPageInfo");

        let user_type = Object::new("User").field(Field::new(
            "name",
            TypeRef::named_nn(TypeRef::STRING),
            |ctx| {
                FieldFuture::new(async move {
                    // The parent_value is the node value itself (stored as FieldValue::Value)
                    let node = ctx.parent_value.as_value().cloned().unwrap_or(Value::Null);
                    Ok(Some(node))
                })
            },
        ));

        let query = Object::new("Query").field(Field::new(
            "users",
            TypeRef::named_nn(conn_builder.connection_type_name()),
            |_ctx| {
                FieldFuture::new(async move {
                    let conn = DynamicConnection::new(false, false)
                        .edge(DynamicEdge::new("c1", Value::from("Alice")))
                        .edge(DynamicEdge::new("c2", Value::from("Bob")));
                    Ok(Some(FieldValue::owned_any(conn)))
                })
            },
        ));

        let schema = Schema::build("Query", None, None)
            .register(user_type)
            .register(conn_builder.page_info_object())
            .register(conn_builder.edge_object())
            .register(conn_builder.connection_object())
            .register(query)
            .finish()
            .unwrap();

        let result = schema
            .execute("{ users { __typename pageInfo { __typename } edges { __typename cursor node { name } } } }")
            .await;

        let data = result.into_result().unwrap().data;
        assert_eq!(
            data,
            value!({
                "users": {
                    "__typename": "UserList",
                    "pageInfo": {"__typename": "UserPageInfo"},
                    "edges": [
                        {"__typename": "UserEntry", "cursor": "c1", "node": {"name": "Alice"}},
                        {"__typename": "UserEntry", "cursor": "c2", "node": {"name": "Bob"}},
                    ]
                }
            })
        );
    }
}
