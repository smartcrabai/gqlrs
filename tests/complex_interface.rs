use gqlrs::*;

#[tokio::test]
pub async fn test_complex_interface_basic() {
    struct Cat {
        id: String,
        name: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }

        async fn meow(&self) -> &str {
            "Meow!"
        }
    }

    struct Dog {
        id: String,
        name: String,
    }

    #[Object]
    impl Dog {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }

        async fn bark(&self) -> &str {
            "Woof!"
        }
    }

    #[derive(Interface)]
    #[graphql(
        complex,
        field(name = "id", ty = "&str"),
        field(name = "name", ty = "&str")
    )]
    enum Animal {
        Cat(Cat),
        Dog(Dog),
    }

    // Define complex interface resolvers for ALL fields
    #[ComplexInterface]
    impl Animal {
        async fn id<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.id.clone()),
                Animal::Dog(dog) => Ok(dog.id.clone()),
            }
        }

        async fn name<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.name.clone()),
                Animal::Dog(dog) => Ok(dog.name.clone()),
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn animal(&self) -> Animal {
            Cat {
                id: "cat-1".to_string(),
                name: "Whiskers".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ animal { id name } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "animal": {
                "id": "cat-1",
                "name": "Whiskers"
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_interface_async_value_resolver() {
    struct Cat {
        id: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }
    }

    #[derive(Interface)]
    #[graphql(complex, field(name = "id", ty = "String"))]
    enum Animal {
        Cat(Cat),
    }

    #[ComplexInterface]
    impl Animal {
        async fn id(&self) -> String {
            match self {
                Animal::Cat(cat) => cat.id.clone(),
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn animal(&self) -> Animal {
            Cat {
                id: "cat-1".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ animal { id } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "animal": {
                "id": "cat-1"
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_interface_dog_variant() {
    struct Cat {
        id: String,
        name: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    struct Dog {
        id: String,
        name: String,
    }

    #[Object]
    impl Dog {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    #[derive(Interface)]
    #[graphql(
        complex,
        field(name = "id", ty = "&str"),
        field(name = "name", ty = "&str")
    )]
    enum Animal {
        Cat(Cat),
        Dog(Dog),
    }

    #[ComplexInterface]
    impl Animal {
        async fn id<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.id.clone()),
                Animal::Dog(dog) => Ok(dog.id.clone()),
            }
        }

        async fn name<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.name.clone()),
                Animal::Dog(dog) => Ok(dog.name.clone()),
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn animal(&self) -> Animal {
            Dog {
                id: "dog-1".to_string(),
                name: "Rex".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ animal { id name } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "animal": {
                "id": "dog-1",
                "name": "Rex"
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_interface_with_arguments() {
    struct Cat {
        id: String,
        name: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    struct Dog {
        id: String,
        name: String,
    }

    #[Object]
    impl Dog {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    #[derive(Interface)]
    #[graphql(
        complex,
        field(name = "id", ty = "&str"),
        field(name = "name", ty = "&str"),
        field(name = "greeting", ty = "String", arg(name = "prefix", ty = "String"))
    )]
    enum Pet {
        Cat(Cat),
        Dog(Dog),
    }

    // Complex interface resolver for ALL fields
    #[ComplexInterface]
    impl Pet {
        async fn id<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Pet::Cat(cat) => Ok(cat.id.clone()),
                Pet::Dog(dog) => Ok(dog.id.clone()),
            }
        }

        async fn name<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Pet::Cat(cat) => Ok(cat.name.clone()),
                Pet::Dog(dog) => Ok(dog.name.clone()),
            }
        }

        async fn greeting(&self, _ctx: &Context<'_>, prefix: String) -> Result<String> {
            let name = match self {
                Pet::Cat(cat) => &cat.name,
                Pet::Dog(dog) => &dog.name,
            };
            Ok(format!("{} {}!", prefix, name))
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn pet(&self) -> Pet {
            Cat {
                id: "cat-1".to_string(),
                name: "Whiskers".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ pet { id greeting(prefix: "Hello") } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "pet": {
                "id": "cat-1",
                "greeting": "Hello Whiskers!"
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_interface_without_complex_attr() {
    // Test that interface works without complex attribute
    struct Cat {
        id: String,
        name: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    struct Dog {
        id: String,
        name: String,
    }

    #[Object]
    impl Dog {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }
    }

    #[derive(Interface)]
    #[graphql(field(name = "id", ty = "&str"), field(name = "name", ty = "&str"))]
    enum Animal {
        Cat(Cat),
        Dog(Dog),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn animal(&self) -> Animal {
            Cat {
                id: "cat-1".to_string(),
                name: "Whiskers".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ animal { id name } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "animal": {
                "id": "cat-1",
                "name": "Whiskers"
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_interface_shared_logic() {
    // Test that complex interface allows sharing common resolution logic
    struct Cat {
        id: String,
        name: String,
        sound: String,
    }

    #[Object]
    impl Cat {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }

        async fn sound(&self) -> &str {
            &self.sound
        }
    }

    struct Dog {
        id: String,
        name: String,
        sound: String,
    }

    #[Object]
    impl Dog {
        async fn id(&self) -> &str {
            &self.id
        }

        async fn name(&self) -> &str {
            &self.name
        }

        async fn sound(&self) -> &str {
            &self.sound
        }
    }

    #[derive(Interface)]
    #[graphql(
        complex,
        field(name = "id", ty = "&str"),
        field(name = "name", ty = "&str"),
        field(name = "sound", ty = "&str"),
        field(name = "info", ty = "String")
    )]
    enum Animal {
        Cat(Cat),
        Dog(Dog),
    }

    #[ComplexInterface]
    impl Animal {
        // Common resolution logic for id - shared across all variants
        async fn id<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.id.clone()),
                Animal::Dog(dog) => Ok(dog.id.clone()),
            }
        }

        // Common resolution logic for name - shared across all variants
        async fn name<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.name.clone()),
                Animal::Dog(dog) => Ok(dog.name.clone()),
            }
        }

        // Common resolution logic for sound - shared across all variants
        async fn sound<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            match self {
                Animal::Cat(cat) => Ok(cat.sound.clone()),
                Animal::Dog(dog) => Ok(dog.sound.clone()),
            }
        }

        // Custom field that doesn't exist on member types
        async fn info<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
            let (name, sound) = match self {
                Animal::Cat(cat) => (&cat.name, &cat.sound),
                Animal::Dog(dog) => (&dog.name, &dog.sound),
            };
            Ok(format!("{} says {}", name, sound))
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn animal(&self) -> Animal {
            Cat {
                id: "cat-1".to_string(),
                name: "Whiskers".to_string(),
                sound: "Meow".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let res = schema
        .execute(r#"{ animal { id name sound info } }"#)
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "animal": {
                "id": "cat-1",
                "name": "Whiskers",
                "sound": "Meow",
                "info": "Whiskers says Meow"
            }
        })
    );
}
