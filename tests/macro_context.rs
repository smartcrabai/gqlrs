//! Regression test for async-graphql#1144: macro-generated resolver context
//! detection.
//!
//! When a type comes through a macro's `:ty` fragment, it gets wrapped in an
//! invisible `Type::Group`. The derive macro must strip these wrappers before
//! checking if an argument is a `&Context`, otherwise the context argument is
//! incorrectly exposed as a GraphQL input field.

use gqlrs::*;

/// Macro that generates an Object impl block with a method taking
/// `&Context<'_>`. The `$ctx_type:ty` fragment causes `Type::Group` wrapping
/// around the type.
macro_rules! define_object_with_ctx {
    ($ctx_type:ty) => {
        struct Query;

        #[gqlrs::Object]
        impl Query {
            async fn value(&self, ctx: $ctx_type) -> i32 {
                ctx.data_unchecked::<i32>() + 1
            }
        }
    };
}

#[tokio::test]
async fn test_macro_ty_context_not_exposed_as_input() {
    // When &Context<'_> goes through :ty, it becomes
    // Type::Group(Type::Reference(...)). Without the fix, the Context argument
    // is incorrectly treated as an input field.
    define_object_with_ctx!(&gqlrs::Context<'_>);

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(10i32)
        .finish();

    let res = schema
        .execute("{ value }")
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(res, value!({ "value": 11 }));
}

/// Macro that generates a ComplexObject impl block with a method taking
/// `&Context<'_>`.
macro_rules! define_complex_object_with_ctx {
    ($ctx_type:ty) => {
        #[derive(gqlrs::SimpleObject)]
        #[graphql(complex)]
        struct MyObj {
            value: i32,
        }

        #[gqlrs::ComplexObject]
        impl MyObj {
            async fn computed(&self, ctx: $ctx_type) -> i32 {
                self.value + ctx.data_unchecked::<i32>()
            }
        }
    };
}

#[tokio::test]
async fn test_macro_ty_context_in_complex_object() {
    define_complex_object_with_ctx!(&gqlrs::Context<'_>);

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj { value: 5 }
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(20i32)
        .finish();

    let res = schema
        .execute("{ obj { computed } }")
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(res, value!({ "obj": { "computed": 25 } }));
}

/// Macro that generates an Object impl with a derived field and `&Context<'_>`.
macro_rules! define_object_with_derived_ctx {
    ($ctx_type:ty) => {
        struct DerivedQuery;

        #[gqlrs::Object]
        impl DerivedQuery {
            #[graphql(derived(name = "value2", into = "i32"))]
            async fn value(&self, ctx: $ctx_type) -> i32 {
                ctx.data_unchecked::<i32>() + 1
            }
        }
    };
}

#[tokio::test]
async fn test_macro_ty_context_in_object_derived_field() {
    define_object_with_derived_ctx!(&gqlrs::Context<'_>);

    let schema = Schema::build(DerivedQuery, EmptyMutation, EmptySubscription)
        .data(10i32)
        .finish();

    let res = schema
        .execute("{ value value2 }")
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(res, value!({ "value": 11, "value2": 11 }));
}

/// Macro that generates a ComplexObject impl with a derived field and
/// `&Context<'_>`.
macro_rules! define_complex_object_with_derived_ctx {
    ($ctx_type:ty) => {
        #[derive(gqlrs::SimpleObject)]
        #[graphql(complex)]
        struct DerivedObj {
            value: i32,
        }

        #[gqlrs::ComplexObject]
        impl DerivedObj {
            #[graphql(derived(name = "computed2", into = "i32"))]
            async fn computed(&self, ctx: $ctx_type) -> i32 {
                self.value + ctx.data_unchecked::<i32>()
            }
        }
    };
}

#[tokio::test]
async fn test_macro_ty_context_in_complex_object_derived_field() {
    define_complex_object_with_derived_ctx!(&gqlrs::Context<'_>);

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> DerivedObj {
            DerivedObj { value: 5 }
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(20i32)
        .finish();

    let res = schema
        .execute("{ obj { computed computed2 } }")
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(res, value!({ "obj": { "computed": 25, "computed2": 25 } }));
}
