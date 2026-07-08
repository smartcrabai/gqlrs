use gqlrs::*;

#[tokio::test]
async fn test_flatten() {
    #[derive(SimpleObject)]
    struct A {
        a: i32,
        b: i32,
    }

    #[derive(SimpleObject)]
    struct B {
        #[graphql(flatten)]
        a: A,
        c: i32,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> B {
            B {
                a: A { a: 100, b: 200 },
                c: 300,
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ __type(name: \"B\") { fields { name } } }";
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "__type": {
                "fields": [
                    {"name": "a"},
                    {"name": "b"},
                    {"name": "c"}
                ]
            }
        })
    );

    let query = "{ obj { a b c } }";
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "obj": {
                "a": 100,
                "b": 200,
                "c": 300,
            }
        })
    );
}

#[tokio::test]
async fn recursive_fragment_definition() {
    #[derive(SimpleObject)]
    struct Hello {
        world: String,
    }

    struct Query;

    // this setup is actually completely irrelevant we just need to be able to
    // execute a query
    #[Object]
    impl Query {
        async fn obj(&self) -> Hello {
            Hello {
                world: "Hello World".into(),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "fragment f on Query {...f} { __typename }";
    assert!(schema.execute(query).await.into_result().is_err());
}

#[tokio::test]
async fn recursive_fragment_definition_nested() {
    #[derive(SimpleObject)]
    struct Hello {
        world: String,
    }

    struct Query;

    // this setup is actually completely irrelevant we just need to be able to
    // execute a query
    #[Object]
    impl Query {
        async fn obj(&self) -> Hello {
            Hello {
                world: "Hello World".into(),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "fragment f on Query { a { ...f a { ...f } } } { __typename }";
    assert!(schema.execute(query).await.into_result().is_err());
}

#[tokio::test]
async fn test_output_using() {
    mod transforms {
        pub fn mask_email(email: String) -> String {
            let parts: Vec<&str> = email.splitn(2, '@').collect();
            if parts.len() == 2 {
                let local = parts[0];
                let domain = parts[1];
                let masked = if local.len() <= 2 {
                    "*".repeat(local.len())
                } else {
                    format!("{}{}", &local[..2], "*".repeat(local.len() - 2))
                };
                format!("{}@{}", masked, domain)
            } else {
                email
            }
        }

        pub fn add_prefix(value: String) -> String {
            format!("prefix_{}", value)
        }
    }

    #[derive(SimpleObject)]
    struct User {
        name: String,
        #[graphql(owned, output_using = "transforms::mask_email")]
        email: String,
        #[graphql(owned, output_using = "transforms::add_prefix")]
        id: String,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn user(&self) -> User {
            User {
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                id: "123".to_string(),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ user { name email id } }";
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "user": {
                "name": "Alice",
                "email": "al***@example.com",
                "id": "prefix_123"
            }
        })
    );
}

#[tokio::test]
async fn test_output_using_borrowed_field() {
    #[derive(SimpleObject)]
    struct Profile {
        label: String,
    }

    fn redact(profile: &Profile) -> Profile {
        Profile {
            label: format!("{}***", &profile.label[..2]),
        }
    }

    #[derive(SimpleObject)]
    struct User {
        #[graphql(output_using = "redact")]
        profile: Profile,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn user(&self) -> User {
            User {
                profile: Profile {
                    label: "secret".to_string(),
                },
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ user { profile { label } } }";
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "user": {
                "profile": {
                    "label": "se***"
                }
            }
        })
    );
}

#[tokio::test]
async fn test_output_using_infers_graphql_type() {
    struct Email(String);

    fn expose(email: &Email) -> String {
        email.0.clone()
    }

    #[derive(SimpleObject)]
    struct User {
        #[graphql(output_using = "expose")]
        email: Email,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn user(&self) -> User {
            User {
                email: Email("alice@example.com".to_string()),
            }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ user { email } }";
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "user": {
                "email": "alice@example.com"
            }
        })
    );
}
