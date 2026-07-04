#![allow(dead_code)]

#[tokio::test]
pub async fn test_object() {
    macro_rules! test_data {
        ($test_name:ident) => {
            #[derive(Debug, Clone)]
            pub struct $test_name {
                value: i32,
            }

            #[gqlrs::Object]
            impl $test_name {
                async fn value(&self) -> i32 {
                    self.value
                }
            }
        };
    }

    test_data!(A);
}

#[tokio::test]
pub async fn test_subscription() {
    macro_rules! test_data {
        ($test_name:ident) => {
            #[derive(Debug, Clone)]
            pub struct $test_name {
                value: i32,
            }

            #[gqlrs::Subscription]
            impl $test_name {
                async fn value(&self) -> impl futures_util::stream::Stream<Item = i32> + 'static {
                    let value = self.value;
                    futures_util::stream::once(async move { value })
                }
            }
        };
    }

    test_data!(A);
}

#[tokio::test]
pub async fn test_scalar() {
    macro_rules! test_data {
        ($test_name:ident) => {
            #[derive(Debug, Clone)]
            pub struct $test_name(i32);

            #[gqlrs::Scalar]
            impl gqlrs::ScalarType for $test_name {
                fn parse(value: gqlrs::Value) -> gqlrs::InputValueResult<Self> {
                    match value {
                        gqlrs::Value::Number(n) if n.is_i64() => {
                            let value = n.as_i64().unwrap();
                            if value < i32::MIN as i64 || value > i32::MAX as i64 {
                                return Err(gqlrs::InputValueError::from("Invalid number"));
                            }
                            Ok($test_name(value as i32))
                        }
                        _ => Err(gqlrs::InputValueError::expected_type(value)),
                    }
                }

                fn to_value(&self) -> gqlrs::Value {
                    self.0.to_value()
                }
            }
        };
    }

    test_data!(A);
}

#[tokio::test]
pub async fn test_oneof_object_type() {
    macro_rules! test_data {
        ($test_name:ident, $type1:ty, $type2:ty) => {
            #[derive(gqlrs::OneofObject)]
            enum $test_name {
                Type1($type1),
                Type2($type2),
            }
        };
    }

    #[derive(gqlrs::InputObject)]
    struct A {
        a: i32,
    }

    #[derive(gqlrs::InputObject)]
    struct B {
        b: i32,
    }

    test_data!(C, A, B);
}
