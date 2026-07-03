use crate::Value;

use super::{Field, FieldFuture, FieldValue, Object, TypeRef};

/// Information about pagination in a connection.
///
/// This is the dynamic-schema equivalent of [`crate::types::connection::PageInfo`].
#[derive(Debug, Clone)]
pub struct DynamicPageInfo {
    /// When paginating backwards, are there more items?
    pub has_previous_page: bool,
    /// When paginating forwards, are there more items?
    pub has_next_page: bool,
    /// When paginating backwards, the cursor to continue.
    pub start_cursor: Option<String>,
    /// When paginating forwards, the cursor to continue.
    pub end_cursor: Option<String>,
}

impl DynamicPageInfo {
    /// Build the dynamic `PageInfo` object type.
    pub fn object_type() -> Object {
        Self::object_type_named("PageInfo")
    }

    fn object_type_named(name: &str) -> Object {
        Object::new(name)
            .field(Field::new(
                "hasPreviousPage",
                TypeRef::named_nn(TypeRef::BOOLEAN),
                |ctx| {
                    FieldFuture::new(async move {
                        let info = ctx.parent_value.try_downcast_ref::<DynamicPageInfo>()?;
                        Ok(Some(Value::from(info.has_previous_page)))
                    })
                },
            ))
            .field(Field::new(
                "hasNextPage",
                TypeRef::named_nn(TypeRef::BOOLEAN),
                |ctx| {
                    FieldFuture::new(async move {
                        let info = ctx.parent_value.try_downcast_ref::<DynamicPageInfo>()?;
                        Ok(Some(Value::from(info.has_next_page)))
                    })
                },
            ))
            .field(Field::new(
                "startCursor",
                TypeRef::named(TypeRef::STRING),
                |ctx| {
                    FieldFuture::new(async move {
                        let info = ctx.parent_value.try_downcast_ref::<DynamicPageInfo>()?;
                        Ok(info.start_cursor.as_ref().map(|c| Value::from(c.clone())))
                    })
                },
            ))
            .field(Field::new(
                "endCursor",
                TypeRef::named(TypeRef::STRING),
                |ctx| {
                    FieldFuture::new(async move {
                        let info = ctx.parent_value.try_downcast_ref::<DynamicPageInfo>()?;
                        Ok(info.end_cursor.as_ref().map(|c| Value::from(c.clone())))
                    })
                },
            ))
    }
}

/// An edge in a dynamic connection.
///
/// Holds a cursor and a node value as a [`Value`] (which is `Clone`).
/// Additional fields can be attached via [`DynamicEdge::extra_field`] and
/// exposed in the schema with [`DynamicConnectionBuilder::edge_field`].
///
/// For simple scalar/object nodes, pass the node as a `Value`. For typed
/// nodes that require `FieldValue::owned_any` or `FieldValue::with_type`,
/// build the edge object type manually with the lower-level dynamic API.
#[derive(Debug, Clone)]
pub struct DynamicEdge {
    /// A cursor for use in pagination.
    pub cursor: String,
    /// The item at the end of the edge, stored as a [`Value`].
    pub node: Value,
    /// Extra fields to attach to the edge object, stored as named [`Value`]s.
    pub extra_fields: Vec<(String, Value)>,
}

impl DynamicEdge {
    /// Create a new edge with a cursor and node value.
    pub fn new(cursor: impl Into<String>, node: impl Into<Value>) -> Self {
        Self {
            cursor: cursor.into(),
            node: node.into(),
            extra_fields: Vec::new(),
        }
    }

    /// Add an extra field to this edge.
    ///
    /// The field is queryable only when the corresponding type field has been
    /// registered with [`DynamicConnectionBuilder::edge_field`]. If the same
    /// name is added more than once, the last value is returned.
    pub fn extra_field(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra_fields.push((name.into(), value.into()));
        self
    }
}

/// A connection result containing edges and page information.
///
/// This is the dynamic-schema equivalent of [`crate::types::connection::Connection`].
///
/// # Examples
///
/// ```
/// use gqlrs::{dynamic::*, Value};
///
/// // Build a connection result.
/// let mut conn = DynamicConnection::new(false, true);
/// for i in 0..3 {
///     conn = conn.edge(DynamicEdge::new(format!("cursor-{i}"), Value::from(i)));
/// }
///
/// assert_eq!(conn.edges.len(), 3);
/// assert_eq!(conn.edges[0].cursor, "cursor-0");
/// ```
#[derive(Debug, Clone)]
pub struct DynamicConnection {
    /// The edges in this connection.
    pub edges: Vec<DynamicEdge>,
    /// Pagination information.
    pub page_info: DynamicPageInfo,
    /// Extra fields on the connection object itself.
    pub extra_fields: Vec<(String, Value)>,
}

impl DynamicConnection {
    /// Create a new connection.
    pub fn new(has_previous_page: bool, has_next_page: bool) -> Self {
        Self {
            edges: Vec::new(),
            page_info: DynamicPageInfo {
                has_previous_page,
                has_next_page,
                start_cursor: None,
                end_cursor: None,
            },
            extra_fields: Vec::new(),
        }
    }

    /// Add an edge to this connection.
    pub fn edge(mut self, edge: DynamicEdge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Add an extra field to the connection object.
    ///
    /// The field is queryable only when the corresponding type field has been
    /// registered with [`DynamicConnectionBuilder::connection_field`]. If the
    /// same name is added more than once, the last value is returned.
    pub fn extra_field(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra_fields.push((name.into(), value.into()));
        self
    }

    /// Return the computed `DynamicPageInfo` for this connection.
    ///
    /// Explicit `start_cursor` / `end_cursor` values on [`Self::page_info`] are
    /// preserved. When they are not set, the first and last edge cursors are
    /// used instead.
    pub fn computed_page_info(&self) -> DynamicPageInfo {
        DynamicPageInfo {
            has_previous_page: self.page_info.has_previous_page,
            has_next_page: self.page_info.has_next_page,
            start_cursor: self
                .page_info
                .start_cursor
                .clone()
                .or_else(|| self.edges.first().map(|e| e.cursor.clone())),
            end_cursor: self
                .page_info
                .end_cursor
                .clone()
                .or_else(|| self.edges.last().map(|e| e.cursor.clone())),
        }
    }

    /// Create a [`DynamicConnectionBuilder`] for building the connection,
    /// edge, and PageInfo object types.
    ///
    /// # Arguments
    ///
    /// * `type_name` - The base type name (e.g., `"Item"`). The builder will
    ///   create `"ItemConnection"` and `"ItemEdge"` by default.
    pub fn builder(type_name: impl Into<String>) -> DynamicConnectionBuilder {
        DynamicConnectionBuilder::new(type_name)
    }
}

/// A builder that assembles the dynamic GraphQL types for a Relay-style
/// connection.
///
/// Use [`DynamicConnection::builder`] to create an instance.
///
/// # Type names
///
/// By default the connection type is named `<node_type_name>Connection` and
/// the edge type is `<node_type_name>Edge`. You can override these with
/// [`connection_name`](Self::connection_name) and [`edge_name`](Self::edge_name).
///
/// # Examples
///
/// ```no_run
/// use gqlrs::{dynamic::*, Value};
///
/// // Define the node type. The node field resolver receives the `Value`
/// // stored in `DynamicEdge::node` as its parent value.
/// let item = Object::new("Item").field(Field::new(
///     "id",
///     TypeRef::named_nn(TypeRef::STRING),
///     |ctx| FieldFuture::new(async move { Ok(ctx.parent_value.as_value().cloned()) }),
/// ));
///
/// // Build connection types and register an additional edge field.
/// let conn_builder = DynamicConnection::builder("Item")
///     .node_type_name(item.type_name())
///     .edge_field("rank", TypeRef::named(TypeRef::INT));
///
/// let mut schema_builder = Schema::build("Query", None, None).register(item);
/// for object in conn_builder.objects() {
///     schema_builder = schema_builder.register(object);
/// }
/// ```
pub struct DynamicConnectionBuilder {
    connection_name: String,
    edge_name: String,
    page_info_name: String,
    node_type: TypeRef,
    edge_fields: Vec<(String, TypeRef)>,
    connection_fields: Vec<(String, TypeRef)>,
}

impl DynamicConnectionBuilder {
    /// Create a new builder with defaults based on the given type name.
    pub fn new(type_name: impl Into<String>) -> Self {
        let type_name = type_name.into();
        Self {
            connection_name: format!("{}Connection", type_name),
            edge_name: format!("{}Edge", type_name),
            page_info_name: "PageInfo".to_string(),
            node_type: TypeRef::named_nn(type_name),
            edge_fields: Vec::new(),
            connection_fields: Vec::new(),
        }
    }

    /// Override the connection type name.
    pub fn connection_name(mut self, name: impl Into<String>) -> Self {
        self.connection_name = name.into();
        self
    }

    /// Override the edge type name.
    pub fn edge_name(mut self, name: impl Into<String>) -> Self {
        self.edge_name = name.into();
        self
    }

    /// Override the PageInfo type name.
    pub fn page_info_name(mut self, name: impl Into<String>) -> Self {
        self.page_info_name = name.into();
        self
    }

    /// Override the node GraphQL type name (the type of the `node` field on
    /// the edge). The type is non-null by default.
    pub fn node_type_name(mut self, name: impl Into<String>) -> Self {
        self.node_type = TypeRef::named_nn(name);
        self
    }

    /// Override the full node GraphQL type reference (the type of the `node`
    /// field on the edge).
    pub fn node_type(mut self, ty: impl Into<TypeRef>) -> Self {
        self.node_type = ty.into();
        self
    }

    /// Add an additional field to the edge type.
    ///
    /// Values are read by name from [`DynamicEdge::extra_fields`].
    pub fn edge_field(mut self, name: impl Into<String>, ty: impl Into<TypeRef>) -> Self {
        self.edge_fields.push((name.into(), ty.into()));
        self
    }

    /// Add an additional field to the connection type.
    ///
    /// Values are read by name from [`DynamicConnection::extra_fields`].
    pub fn connection_field(mut self, name: impl Into<String>, ty: impl Into<TypeRef>) -> Self {
        self.connection_fields.push((name.into(), ty.into()));
        self
    }

    /// Returns the connection type name that this builder will produce.
    pub fn connection_type_name(&self) -> &str {
        &self.connection_name
    }

    /// Returns the edge type name that this builder will produce.
    pub fn edge_type_name(&self) -> &str {
        &self.edge_name
    }

    /// Returns the PageInfo type name that this builder will produce.
    pub fn page_info_type_name(&self) -> &str {
        &self.page_info_name
    }

    /// Build and return the three [`Object`] types (PageInfo, Edge,
    /// Connection) that should be registered on the schema.
    pub fn objects(&self) -> Vec<Object> {
        vec![
            self.page_info_object(),
            self.edge_object(),
            self.connection_object(),
        ]
    }

    /// Build the PageInfo object type.
    pub fn page_info_object(&self) -> Object {
        DynamicPageInfo::object_type_named(&self.page_info_name)
    }

    /// Build the Edge object type.
    pub fn edge_object(&self) -> Object {
        let node_type = self.node_type.clone();
        let mut object = Object::new(&self.edge_name)
            .field(Field::new(
                "cursor",
                TypeRef::named_nn(TypeRef::STRING),
                |ctx| {
                    FieldFuture::new(async move {
                        let edge = ctx.parent_value.try_downcast_ref::<DynamicEdge>()?;
                        Ok(Some(Value::from(edge.cursor.clone())))
                    })
                },
            ))
            .field(Field::new("node", node_type, |ctx| {
                FieldFuture::new(async move {
                    let edge = ctx.parent_value.try_downcast_ref::<DynamicEdge>()?;
                    Ok(Some(FieldValue::value(edge.node.clone())))
                })
            }));

        for (name, ty) in &self.edge_fields {
            let field_name = name.clone();
            object = object.field(Field::new(name.clone(), ty.clone(), move |ctx| {
                let field_name = field_name.clone();
                FieldFuture::new(async move {
                    let edge = ctx.parent_value.try_downcast_ref::<DynamicEdge>()?;
                    Ok(extra_field_value(&edge.extra_fields, &field_name))
                })
            }));
        }

        object
    }

    /// Build the Connection object type.
    pub fn connection_object(&self) -> Object {
        let page_info_name = self.page_info_name.clone();
        let edge_type_name = self.edge_name.clone();
        let edge_type_ref = TypeRef::named_nn_list_nn(&edge_type_name);
        let nodes_type_ref = non_null_list(self.node_type.clone());

        let mut object = Object::new(&self.connection_name)
            .field(Field::new(
                "pageInfo",
                TypeRef::named_nn(page_info_name),
                |ctx| {
                    FieldFuture::new(async move {
                        let conn = ctx.parent_value.try_downcast_ref::<DynamicConnection>()?;
                        Ok(Some(FieldValue::owned_any(conn.computed_page_info())))
                    })
                },
            ))
            .field(Field::new("edges", edge_type_ref, |ctx| {
                FieldFuture::new(async move {
                    let conn = ctx.parent_value.try_downcast_ref::<DynamicConnection>()?;
                    let edges = conn.edges.iter().cloned().map(FieldValue::owned_any);
                    Ok(Some(FieldValue::list(edges)))
                })
            }))
            .field(Field::new("nodes", nodes_type_ref, |ctx| {
                FieldFuture::new(async move {
                    let conn = ctx.parent_value.try_downcast_ref::<DynamicConnection>()?;
                    let nodes = conn
                        .edges
                        .iter()
                        .map(|edge| FieldValue::value(edge.node.clone()));
                    Ok(Some(FieldValue::list(nodes)))
                })
            }));

        for (name, ty) in &self.connection_fields {
            let field_name = name.clone();
            object = object.field(Field::new(name.clone(), ty.clone(), move |ctx| {
                let field_name = field_name.clone();
                FieldFuture::new(async move {
                    let conn = ctx.parent_value.try_downcast_ref::<DynamicConnection>()?;
                    Ok(extra_field_value(&conn.extra_fields, &field_name))
                })
            }));
        }

        object
    }
}

fn extra_field_value<'a>(fields: &[(String, Value)], field_name: &str) -> Option<FieldValue<'a>> {
    fields
        .iter()
        .rev()
        .find(|(name, _)| name == field_name)
        .map(|(_, value)| FieldValue::value(value.clone()))
}

fn non_null_list(item_type: TypeRef) -> TypeRef {
    TypeRef::NonNull(Box::new(TypeRef::List(Box::new(item_type))))
}
