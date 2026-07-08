Define resolvers for an interface type at the interface level.

This allows defining common field resolvers directly on the interface enum,
so you don't need to repeat the same field implementation for every member type.

When using `ComplexInterface`, you must use the `complex` attribute on the `Interface` derive,
and provide resolvers for ALL interface fields in the `ComplexInterface` impl.

# Macro attributes

| Attribute     | description                                                                                                                              | Type   | Optional |
|---------------|------------------------------------------------------------------------------------------------------------------------------------------|--------|----------|
| rename_fields | Rename all the fields according to the given case convention. Possible values: "lowercase", "UPPERCASE", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE" | string | Y        |
| rename_args   | Rename all the arguments according to the given case convention. Possible values: "lowercase", "UPPERCASE", "PascalCase", "camelCase", "snake_case", "SCREAMING_SNAKE_CASE" | string | Y        |

# Field attributes

| Attribute | description                                         | Type   | Optional |
|-----------|-----------------------------------------------------|--------|----------|
| skip      | Skip this field                                     | bool   | Y        |
| name      | Field name, defaults to the Rust method name        | string | Y        |

# Example

```rust
use gqlrs::*;

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

// Note: the `complex` attribute enables ComplexInterface support
#[derive(Interface)]
#[graphql(
    complex,
    field(name = "id", ty = "&str"),
    field(name = "name", ty = "&str"),
)]
enum Animal {
    Cat(Cat),
    Dog(Dog),
}

// ComplexInterface resolvers for ALL interface fields
#[ComplexInterface]
impl Animal {
    // Resolve the id field at the interface level
    async fn id<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
        match self {
            Animal::Cat(cat) => Ok(cat.id.clone()),
            Animal::Dog(dog) => Ok(dog.id.clone()),
        }
    }

    // Resolve the name field at the interface level
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

# tokio::runtime::Runtime::new().unwrap().block_on(async move {
let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
let res = schema
    .execute(r#"{ animal { id name } }"#)
    .await
    .into_result()
    .unwrap()
    .data;
assert_eq!(res, value!({
    "animal": {
        "id": "cat-1",
        "name": "Whiskers"
    }
}));
# });
```

# Custom fields

You can also define custom fields that don't exist on the member types:

```rust
use gqlrs::*;

struct Cat {
    id: String,
    name: String,
}

#[Object]
impl Cat {
    async fn id(&self) -> &str { &self.id }
    async fn name(&self) -> &str { &self.name }
}

struct Dog {
    id: String,
    name: String,
}

#[Object]
impl Dog {
    async fn id(&self) -> &str { &self.id }
    async fn name(&self) -> &str { &self.name }
}

#[derive(Interface)]
#[graphql(
    complex,
    field(name = "id", ty = "&str"),
    field(name = "name", ty = "&str"),
    field(name = "info", ty = "String"),
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

    // Custom field that doesn't exist on member types
    async fn info<'a>(&self, _ctx: &Context<'a>) -> Result<String> {
        let name = match self {
            Animal::Cat(cat) => &cat.name,
            Animal::Dog(dog) => &dog.name,
        };
        Ok(format!("Animal: {}", name))
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

# tokio::runtime::Runtime::new().unwrap().block_on(async move {
let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
let res = schema
    .execute(r#"{ animal { id info } }"#)
    .await
    .into_result()
    .unwrap()
    .data;
assert_eq!(res, value!({
    "animal": {
        "id": "cat-1",
        "info": "Animal: Whiskers"
    }
}));
# });
```
