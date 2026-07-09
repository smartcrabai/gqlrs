use gqlrs::*;

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

#[cfg_attr(
    all(feature = "boxed-trait", not(feature = "no_send")),
    async_trait::async_trait
)]
#[cfg_attr(all(feature = "boxed-trait", feature = "no_send"), async_trait::async_trait(?Send))]
impl Guard for RoleGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        if ctx.data_opt::<Role>() == Some(&self.role) {
            Ok(())
        } else {
            Err("Forbidden".into())
        }
    }
}

#[derive(InputObject)]
struct AdminInput {
    #[graphql(guard = "RoleGuard::new(Role::Admin)")]
    admin_only_field: Option<String>,
    normal_field: String,
}

#[derive(InputObject)]
struct NestedInput {
    #[graphql(guard = "RoleGuard::new(Role::Admin)")]
    admin_only_field: Option<String>,
}

#[derive(InputObject)]
struct WrapperInput {
    nested: Option<NestedInput>,
    nested_list: Option<Vec<NestedInput>>,
}

#[derive(InputObject)]
#[graphql(concrete(name = "ConcreteGuardInput", params(String)))]
struct GenericGuardInput<T: InputType> {
    #[graphql(guard = "RoleGuard::new(Role::Admin)")]
    admin_only_field: Option<T>,
}

#[derive(OneofObject)]
enum OneofWrapperInput {
    Nested(NestedInput),
}

struct Query;

#[Object]
impl Query {
    async fn process_input(&self, _input: AdminInput) -> String {
        "processed".to_string()
    }

    async fn process_boxed_input(&self, _input: Box<AdminInput>) -> String {
        "processed".to_string()
    }

    async fn process_wrapper_input(&self, _input: WrapperInput) -> String {
        "processed".to_string()
    }

    async fn process_generic_input(&self, _input: GenericGuardInput<String>) -> String {
        "processed".to_string()
    }

    async fn process_oneof_input(&self, _input: OneofWrapperInput) -> String {
        "processed".to_string()
    }
}

#[tokio::test]
async fn test_input_object_field_guard_admin_allowed() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processInput(input: { adminOnlyField: "secret", normalField: "normal" }) }"#;
    assert_eq!(
        schema
            .execute(Request::new(query).data(Role::Admin))
            .await
            .data,
        value!({"processInput": "processed"})
    );
}

#[tokio::test]
async fn test_input_object_field_guard_guest_rejected() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processInput(input: { adminOnlyField: "secret", normalField: "normal" }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    // The guard should reject the input
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_input_object_field_guard_guest_without_guarded_field() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processInput(input: { normalField: "normal" }) }"#;
    assert_eq!(
        schema
            .execute(Request::new(query).data(Role::Guest))
            .await
            .data,
        value!({"processInput": "processed"})
    );
}

#[tokio::test]
async fn test_input_object_field_guard_guest_with_null_guarded_field() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processInput(input: { adminOnlyField: null, normalField: "normal" }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_input_object_field_guard_boxed_input() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query =
        r#"{ processBoxedInput(input: { adminOnlyField: "secret", normalField: "normal" }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_nested_input_object_field_guard() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processWrapperInput(input: { nested: { adminOnlyField: "secret" } }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_nested_input_object_field_guard_without_guarded_field() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processWrapperInput(input: { nested: {} }) }"#;
    assert_eq!(
        schema
            .execute(Request::new(query).data(Role::Guest))
            .await
            .data,
        value!({"processWrapperInput": "processed"})
    );
}

#[tokio::test]
async fn test_list_input_object_field_guard() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processWrapperInput(input: { nestedList: [{ adminOnlyField: "secret" }] }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_concrete_input_object_field_guard() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processGenericInput(input: { adminOnlyField: "secret" }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}

#[tokio::test]
async fn test_oneof_nested_input_object_field_guard() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let query = r#"{ processOneofInput(input: { nested: { adminOnlyField: "secret" } }) }"#;
    let result = schema.execute(Request::new(query).data(Role::Guest)).await;
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].message, "Forbidden");
}
