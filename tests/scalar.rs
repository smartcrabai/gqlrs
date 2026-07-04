#![allow(clippy::diverging_sub_expression)]

use gqlrs::*;

mod test_mod {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct MyValue {
        a: i32,
    }
}

mod generic_mod {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct Bar {
        value: i32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Foo<T> {
        value: T,
    }
}

scalar!(
    test_mod::MyValue,
    "MV",
    "DESC",
    "https://tools.ietf.org/html/rfc4122"
);
scalar!(generic_mod::Foo<generic_mod::Bar>);

#[tokio::test]
pub async fn test_scalar_macro() {
    struct Query;

    #[Object]
    #[allow(unreachable_code)]
    impl Query {
        async fn value(&self) -> test_mod::MyValue {
            todo!()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema
            .execute(r#"{ __type(name:"MV") { name description specifiedByURL } }"#)
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "__type": {
                "name": "MV",
                "description": "DESC",
                "specifiedByURL": "https://tools.ietf.org/html/rfc4122",
            }
        })
    );
}

#[tokio::test]
pub async fn test_scalar_macro_default_name_for_generic_type() {
    type GenericScalar = generic_mod::Foo<generic_mod::Bar>;

    assert_eq!(<GenericScalar as InputType>::type_name().as_ref(), "FooBar");
    assert_eq!(
        <GenericScalar as OutputType>::type_name().as_ref(),
        "FooBar"
    );

    struct Query;

    #[Object]
    #[allow(unreachable_code)]
    impl Query {
        async fn value(&self) -> GenericScalar {
            todo!()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema
            .execute(r#"{ __type(name:"FooBar") { name } }"#)
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "__type": {
                "name": "FooBar",
            }
        })
    );
}

#[tokio::test]
pub async fn test_float_inf() {
    struct Query;

    #[Object]
    impl Query {
        async fn value(&self) -> f32 {
            f32::INFINITY
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema
            .execute("{ value }")
            .await
            .into_result()
            .unwrap()
            .data,
        value!({ "value": null })
    );
}
