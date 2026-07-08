use core::marker::PhantomData;

use gqlrs::*;

#[tokio::test]
async fn test_complex_object_process_with_method_field() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct MyObj {
        a: i32,
    }

    #[ComplexObject]
    impl MyObj {
        async fn test(
            &self,
            #[graphql(process_with = "str::make_ascii_uppercase")] processed_complex_arg: String,
        ) -> String {
            processed_complex_arg
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj { a: 10 }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ obj { test(processedComplexArg: \"smol\") } }";
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "obj": {
                "test": "SMOL"
            }
        })
    );
}

#[tokio::test]
async fn test_complex_object_non_async_resolvers() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct MyObj {
        value: i32,
    }

    #[ComplexObject]
    impl MyObj {
        fn a(&self) -> i32 {
            self.value + 1
        }

        fn b(&self, ctx: &Context<'_>, v: i32) -> Result<i32> {
            Ok(self.value + v + ctx.data::<i32>().unwrap())
        }

        fn c(&self) -> Result<bool> {
            Ok(true)
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj { value: 10 }
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(30i32)
        .finish();
    let res = schema
        .execute("{ obj { a b(v: 2) c } }")
        .await
        .into_result()
        .unwrap()
        .data;
    assert_eq!(
        res,
        value!({
            "obj": {
                "a": 11,
                "b": 42,
                "c": true,
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_object() {
    /// A complex object.
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct MyObj {
        a: i32,
        b: i32,
    }

    #[ComplexObject]
    impl MyObj {
        /// A field named `c`.
        async fn c(&self) -> i32 {
            self.a + self.b
        }

        /// A field named `d`.
        async fn d(&self, #[graphql(desc = "An argument named `v`.")] v: i32) -> i32 {
            self.a + self.b + v
        }
    }

    #[allow(clippy::duplicated_attributes)]
    #[derive(Interface)]
    #[graphql(
        field(name = "a", ty = "&i32"),
        field(name = "b", ty = "&i32"),
        field(name = "c", ty = "i32"),
        field(name = "d", ty = "i32", arg(name = "v", ty = "i32"))
    )]
    enum ObjInterface {
        MyObj(MyObj),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj { a: 10, b: 20 }
        }

        async fn obj2(&self) -> ObjInterface {
            MyObj { a: 10, b: 20 }.into()
        }
    }

    let query = "{ obj { a b c d(v:100) } obj2 { a b c d(v:200) } }";
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.data,
        value!({
            "obj": {
                "a": 10,
                "b": 20,
                "c": 30,
                "d": 130,
            },
            "obj2": {
                "a": 10,
                "b": 20,
                "c": 30,
                "d": 230,
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_object_with_generic_context_data() {
    trait MyData: Send + Sync {
        fn answer(&self) -> i32;
    }

    struct DefaultMyData {}

    impl MyData for DefaultMyData {
        fn answer(&self) -> i32 {
            42
        }
    }

    struct MyQuery<D: MyData> {
        marker: PhantomData<D>,
    }

    #[Object]
    impl<D> MyQuery<D>
    where
        D: 'static + MyData,
    {
        #[graphql(skip)]
        pub fn new() -> Self {
            Self {
                marker: PhantomData,
            }
        }

        async fn obj(&self, ctx: &Context<'_>) -> MyObject<D> {
            MyObject::new(ctx.data::<D>().unwrap().answer())
        }
    }

    #[derive(SimpleObject, Debug, Clone, Hash, Eq, PartialEq)]
    #[graphql(complex)]
    struct MyObject<D: MyData> {
        my_val: i32,
        #[graphql(skip)]
        marker: PhantomData<D>,
    }

    #[ComplexObject]
    impl<D: MyData> MyObject<D> {
        #[graphql(skip)]
        pub fn new(my_val: i32) -> Self {
            Self {
                my_val,
                marker: PhantomData,
            }
        }
    }

    let schema = Schema::build(
        MyQuery::<DefaultMyData>::new(),
        EmptyMutation,
        EmptySubscription,
    )
    .data(DefaultMyData {})
    .finish();

    assert_eq!(
        schema.execute("{ obj { myVal } }").await.data,
        value!({
            "obj": {
                "myVal": 42,
            }
        })
    );
}

#[tokio::test]
pub async fn test_complex_object_with_generic_concrete_type() {
    #[derive(SimpleObject)]
    #[graphql(concrete(name = "MyObjIntString", params(i32, String)))]
    #[graphql(concrete(name = "MyObji16u8", params(i16, u8)))]
    #[graphql(complex)]
    struct MyObj<A: OutputType, B: OutputType> {
        a: A,
        b: B,
    }

    #[ComplexObject]
    impl MyObj<i32, String> {
        async fn value_a(&self) -> String {
            format!("i32,String {},{}", self.a, self.b)
        }
    }

    #[ComplexObject]
    impl MyObj<i16, u8> {
        async fn value_b(&self) -> String {
            format!("i16,u8 {},{}", self.a, self.b)
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn q1(&self) -> MyObj<i32, String> {
            MyObj {
                a: 100,
                b: "abc".to_string(),
            }
        }

        async fn q2(&self) -> MyObj<i16, u8> {
            MyObj { a: 100, b: 28 }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    let query = "{ q1 { a b valueA } q2 { a b valueB } }";
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "q1": {
                "a": 100,
                "b": "abc",
                "valueA": "i32,String 100,abc",
            },
            "q2": {
                "a": 100,
                "b": 28,
                "valueB": "i16,u8 100,28",
            }
        })
    );

    assert_eq!(
        schema
            .execute(r#"{ __type(name: "MyObjIntString") { fields { name type { kind ofType { name } } } } }"#)
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "__type": {
                "fields": [
                    {
                        "name": "a",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "Int" },
                        },
                    },
                    {
                        "name": "b",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "String" },
                        },
                    },
                    {
                        "name": "valueA",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "String" },
                        },
                    },
                ]
            }
        })
    );

    assert_eq!(
        schema
            .execute(r#"{ __type(name: "MyObji16u8") { fields { name type { kind ofType { name } } } } }"#)
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "__type": {
                "fields": [
                    {
                        "name": "a",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "Int" },
                        },
                    },
                    {
                        "name": "b",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "Int" },
                        },
                    },
                    {
                        "name": "valueB",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "String" },
                        },
                    },
                ]
            }
        })
    );

    assert_eq!(
        schema
            .execute(
                r#"{ __type(name: "Query") { fields { name type { kind ofType { name } } } } }"#
            )
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "__type": {
                "fields": [
                    {
                        "name": "q1",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "MyObjIntString" },
                        },
                    },
                    {
                        "name": "q2",
                        "type": {
                            "kind": "NON_NULL",
                            "ofType": { "name": "MyObji16u8" },
                        },
                    },
                ]
            }
        })
    );
}

#[tokio::test]
async fn test_flatten() {
    #[derive(SimpleObject)]
    struct A {
        a: i32,
        b: i32,
    }

    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct B {
        #[graphql(skip)]
        a: A,
        c: i32,
    }

    #[ComplexObject]
    impl B {
        #[graphql(flatten)]
        async fn a(&self) -> &A {
            &self.a
        }
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
                    {"name": "c"},
                    {"name": "a"},
                    {"name": "b"}
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
async fn test_flatten_with_generics() {
    #[derive(SimpleObject)]
    struct A {
        a: i32,
        b: i32,
    }

    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct B<T: ObjectType, C: OutputType> {
        #[graphql(skip)]
        a: T,
        c: C,
    }

    #[ComplexObject]
    impl<T: ObjectType, C: OutputType> B<T, C> {
        #[graphql(flatten)]
        async fn a(&self) -> &T {
            &self.a
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> B<A, i32> {
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
                    {"name": "c"},
                    {"name": "a"},
                    {"name": "b"}
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
async fn test_flatten_with_context() {
    #[derive(SimpleObject)]
    struct A {
        a: i32,
        b: i32,
    }

    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct B {
        #[graphql(skip)]
        a: A,
        c: i32,
    }

    #[ComplexObject]
    impl B {
        #[graphql(flatten)]
        async fn a(&self, _ctx: &Context<'_>) -> &A {
            &self.a
        }
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
                    {"name": "c"},
                    {"name": "a"},
                    {"name": "b"}
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
async fn test_flatten_with_result() {
    #[derive(SimpleObject)]
    struct A {
        a: i32,
        b: i32,
    }

    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct B {
        #[graphql(skip)]
        a: A,
        c: i32,
    }

    #[ComplexObject]
    impl B {
        #[graphql(flatten)]
        async fn a(&self) -> FieldResult<&A> {
            Ok(&self.a)
        }
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
                    {"name": "c"},
                    {"name": "a"},
                    {"name": "b"}
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

// Regression test for async-graphql#1549: ComplexObject resolver returning
// Result<Option<T>> where an Err should null only that nullable field,
// not the entire response.
#[tokio::test]
async fn test_complex_object_nullable_result_error_does_not_null_ancestor() {
    #[derive(SimpleObject)]
    #[graphql(complex)]
    struct MyObj {
        a: i32,
    }

    #[ComplexObject]
    impl MyObj {
        // Nullable async field that returns an error.
        async fn nullable_status(&self) -> Result<Option<String>> {
            Err("something went wrong".into())
        }

        // Non-nullable async field that returns an error.
        async fn non_nullable_status(&self) -> Result<String> {
            Err("something went wrong".into())
        }

        // Nullable async field with an argument validator.
        async fn nullable_with_arg(
            &self,
            #[graphql(validator(maximum = 10))] n: i32,
        ) -> Option<i32> {
            Some(n)
        }

        // Nullable sync field that returns an error.
        fn nullable_sync_status(&self) -> Result<Option<String>> {
            Err("something went wrong".into())
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn obj(&self) -> MyObj {
            MyObj { a: 10 }
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    // When a nullable async resolver errors, only that field should be null
    // and the rest of the response should still be valid.
    let query = "{ obj { a nullableStatus } }";
    let response = schema.execute(query).await;
    assert_eq!(
        response.data,
        value!({
            "obj": {
                "a": 10,
                "nullableStatus": null,
            }
        })
    );
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "something went wrong");

    // When a non-nullable async resolver errors, the error propagates up and
    // nulls an ancestor (the whole "obj" here becomes null or the response data
    // is an error).
    let query = "{ obj { a nonNullableStatus } }";
    let response = schema.execute(query).await;
    // Non-nullable field error should propagate, nulling an ancestor.
    assert!(response.errors.len() >= 1);

    // Argument/validator failures are not resolver errors; even for a nullable
    // field they should keep propagating instead of being converted to a field
    // null.
    let query = "{ obj { a nullableWithArg(n: 11) } }";
    let response = schema.execute(query).await;
    assert_eq!(response.data, Value::Null);
    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].path,
        vec![
            PathSegment::Field("obj".to_string()),
            PathSegment::Field("nullableWithArg".to_string()),
        ]
    );

    // Nullable sync resolver that errors should also only null the field.
    let query = "{ obj { a nullableSyncStatus } }";
    let response = schema.execute(query).await;
    assert_eq!(
        response.data,
        value!({
            "obj": {
                "a": 10,
                "nullableSyncStatus": null,
            }
        })
    );
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "something went wrong");
}
