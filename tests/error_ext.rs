#![allow(dead_code)]

use gqlrs::*;

#[tokio::test]
pub async fn test_error_extensions() {
    #[derive(Enum, Eq, PartialEq, Copy, Clone)]
    enum MyEnum {
        Create,
        Delete,
        Update,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn extend_err(&self) -> Result<i32> {
            Err("my error".extend_with(|err, e| {
                e.set("msg", err.to_string());
                e.set("code", 100);
                e.set("action", MyEnum::Create)
            }))
        }

        async fn extend_result(&self) -> Result<i32> {
            Err(Error::from("my error"))
                .extend_err(|_, e| {
                    e.set("msg", "my error");
                    e.set("code", 100);
                })
                .extend_err(|_, e| {
                    e.set("code2", 20);
                })
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    assert_eq!(
        serde_json::to_value(&schema.execute("{ extendErr }").await).unwrap(),
        serde_json::json!({
            "data": {"extendErr": null},
            "errors": [{
                "message": "my error",
                "locations": [{
                    "column": 3,
                    "line": 1,
                }],
                "path": ["extendErr"],
                "extensions": {
                    "msg": "my error",
                    "code": 100,
                    "action": "CREATE",
                }
            }]
        })
    );

    assert_eq!(
        serde_json::to_value(&schema.execute("{ extendResult }").await).unwrap(),
        serde_json::json!({
            "data": {"extendResult": null},
            "errors": [{
                "message": "my error",
                "locations": [{
                    "column": 3,
                    "line": 1,
                }],
                "path": ["extendResult"],
                "extensions": {
                    "msg": "my error",
                    "code": 100,
                    "code2": 20
                }
            }]
        })
    );
}

#[tokio::test]
pub async fn test_failure() {
    #[derive(thiserror::Error, Debug, PartialEq)]
    enum MyError {
        #[error("error1")]
        Error1,

        #[error("error2")]
        Error2,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn failure(&self) -> Result<i32> {
            Err(Error::new_with_source(MyError::Error1))
        }

        async fn failure2(&self) -> Result<i32> {
            Err(Error::new_with_source(MyError::Error2))
        }

        async fn failure3(&self) -> Result<i32> {
            Err(Error::new_with_source(MyError::Error1)
                .extend_with(|_, values| values.set("a", 1))
                .extend_with(|_, values| values.set("b", 2)))
        }

        async fn failure4(&self) -> Result<i32> {
            Err(Error::new_with_source(MyError::Error2))
                .extend_err(|_, values| values.set("a", 1))
                .extend_err(|_, values| values.set("b", 2))?;
            Ok(1)
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let err = schema
        .execute("{ failure }")
        .await
        .into_result()
        .unwrap_err()
        .remove(0);
    assert_eq!(err.source::<MyError>().unwrap(), &MyError::Error1);

    let err = schema
        .execute("{ failure2 }")
        .await
        .into_result()
        .unwrap_err()
        .remove(0);
    assert_eq!(err.source::<MyError>().unwrap(), &MyError::Error2);

    let err = schema
        .execute("{ failure3 }")
        .await
        .into_result()
        .unwrap_err()
        .remove(0);
    assert_eq!(err.source::<MyError>().unwrap(), &MyError::Error1);
    assert_eq!(
        err.extensions,
        Some({
            let mut values = ErrorExtensionValues::default();
            values.set("a", 1);
            values.set("b", 2);
            values
        })
    );

    let err = schema
        .execute("{ failure4 }")
        .await
        .into_result()
        .unwrap_err()
        .remove(0);
    assert_eq!(err.source::<MyError>().unwrap(), &MyError::Error2);
    assert_eq!(
        err.extensions,
        Some({
            let mut values = ErrorExtensionValues::default();
            values.set("a", 1);
            values.set("b", 2);
            values
        })
    );
}

#[tokio::test]
pub async fn test_failure2() {
    #[derive(thiserror::Error, Debug, PartialEq)]
    enum MyError {
        #[error("error1")]
        Error1,
    }

    impl From<MyError> for Error {
        fn from(e: MyError) -> Self {
            Error::new_with_source(e)
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn failure(&self) -> Result<i32> {
            Err(Error::new_with_source(MyError::Error1))
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let err = schema
        .execute("{ failure }")
        .await
        .into_result()
        .unwrap_err()
        .remove(0);
    assert_eq!(err.source::<MyError>().unwrap(), &MyError::Error1);
}

/// Test that custom error types can supply extensions automatically via
/// `IntoError`.
#[tokio::test]
pub async fn test_into_error_with_extensions() {
    #[derive(Debug)]
    enum AppError {
        NotFound { entity: String, id: String },
        Unauthorized,
    }

    impl std::fmt::Display for AppError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                AppError::NotFound { entity, id } => {
                    write!(f, "{} with id '{}' not found", entity, id)
                }
                AppError::Unauthorized => write!(f, "Unauthorized access"),
            }
        }
    }

    impl IntoError for AppError {
        fn into_error(self) -> Error {
            let (message, extensions) = match &self {
                AppError::NotFound { entity, id } => {
                    let mut ext = ErrorExtensionValues::default();
                    ext.set("code", "NOT_FOUND");
                    ext.set("entity", entity.clone());
                    ext.set("entityId", id.clone());
                    (format!("{} with id '{}' not found", entity, id), Some(ext))
                }
                AppError::Unauthorized => {
                    let mut ext = ErrorExtensionValues::default();
                    ext.set("code", "UNAUTHORIZED");
                    ("Unauthorized access".to_string(), Some(ext))
                }
            };
            Error {
                message,
                source: Some(std::sync::Arc::new(self)),
                extensions,
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn find_user(&self) -> Result<String, AppError> {
            Err(AppError::NotFound {
                entity: "User".to_string(),
                id: "123".to_string(),
            })
        }

        async fn secret(&self) -> Result<String, AppError> {
            Err(AppError::Unauthorized)
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // Test NotFound error with extensions
    let result = schema.execute("{ findUser }").await;
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({
            "data": {"findUser": null},
            "errors": [{
                "message": "User with id '123' not found",
                "locations": [{ "column": 3, "line": 1 }],
                "path": ["findUser"],
                "extensions": {
                    "code": "NOT_FOUND",
                    "entity": "User",
                    "entityId": "123"
                }
            }]
        })
    );

    // Test Unauthorized error with extensions
    let result = schema.execute("{ secret }").await;
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({
            "data": {"secret": null},
            "errors": [{
                "message": "Unauthorized access",
                "locations": [{ "column": 3, "line": 1 }],
                "path": ["secret"],
                "extensions": {
                    "code": "UNAUTHORIZED"
                }
            }]
        })
    );
}

/// Test that `IntoError` errors can be used with `?` operator in resolvers.
#[tokio::test]
pub async fn test_into_error_with_question_mark() {
    #[derive(Debug)]
    enum ServiceError {
        DatabaseError(String),
    }

    impl std::fmt::Display for ServiceError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ServiceError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            }
        }
    }

    impl IntoError for ServiceError {
        fn into_error(self) -> Error {
            let mut ext = ErrorExtensionValues::default();
            ext.set("code", "DATABASE_ERROR");
            match &self {
                ServiceError::DatabaseError(msg) => {
                    ext.set("details", msg.clone());
                }
            }
            Error {
                message: self.to_string(),
                source: Some(std::sync::Arc::new(self)),
                extensions: Some(ext),
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn data(&self) -> Result<String> {
            // Using ? operator - extensions are automatically included
            Err(ServiceError::DatabaseError(
                "connection timeout".to_string(),
            ))?
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let result = schema.execute("{ data }").await;

    let err = result.into_result().unwrap_err().remove(0);
    assert_eq!(err.message, "Database error: connection timeout");
    assert!(err.source::<ServiceError>().is_some());

    let extensions = err.extensions.unwrap();
    assert_eq!(extensions.get("code"), Some(&Value::from("DATABASE_ERROR")));
    assert_eq!(
        extensions.get("details"),
        Some(&Value::from("connection timeout"))
    );
}
