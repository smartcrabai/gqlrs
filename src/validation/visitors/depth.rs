use async_graphql_parser::types::Field;

use crate::{
    Positioned,
    registry::MetaTypeName,
    validation::visitor::{VisitMode, Visitor, VisitorContext},
};

pub struct DepthCalculate<'a> {
    max_depth: &'a mut usize,
    current_depth: usize,
}

impl<'a> DepthCalculate<'a> {
    pub fn new(max_depth: &'a mut usize) -> Self {
        Self {
            max_depth,
            current_depth: 0,
        }
    }

    fn field_depth_cost(ctx: &VisitorContext<'_>, field: &Positioned<Field>) -> usize {
        ctx.parent_type()
            .and_then(|ty| {
                ty.field_by_name(MetaTypeName::concrete_typename(
                    field.node.name.node.as_str(),
                ))
            })
            .map_or(1, |meta_field| meta_field.depth_cost)
    }
}

impl<'ctx> Visitor<'ctx> for DepthCalculate<'_> {
    fn mode(&self) -> VisitMode {
        VisitMode::Inline
    }

    fn enter_field(&mut self, ctx: &mut VisitorContext<'ctx>, field: &'ctx Positioned<Field>) {
        let cost = Self::field_depth_cost(ctx, field);
        self.current_depth += cost;
        *self.max_depth = (*self.max_depth).max(self.current_depth);
        ctx.check_depth_limit(self.current_depth);
    }

    fn exit_field(&mut self, ctx: &mut VisitorContext<'ctx>, field: &'ctx Positioned<Field>) {
        self.current_depth -= Self::field_depth_cost(ctx, field);
    }
}

#[cfg(test)]
#[allow(clippy::diverging_sub_expression)]
mod tests {
    use futures_util::stream::BoxStream;

    use super::*;
    use crate::{
        EmptyMutation, EmptySubscription, Interface, Object, Schema, SimpleObject, Subscription,
        parser::parse_query, validation::visit,
    };

    struct Query;

    #[derive(SimpleObject)]
    #[graphql(internal)]
    struct MySimpleObj {
        #[graphql(depth_cost = 0)]
        cheap: i32,
        #[graphql(depth_cost = 2)]
        expensive: i32,
    }

    #[derive(SimpleObject)]
    #[graphql(internal)]
    struct InterfaceObj {
        value: i32,
        #[graphql(depth_cost = 0)]
        specific: i32,
    }

    #[derive(Interface)]
    #[graphql(internal, field(name = "value", ty = "&i32", depth_cost = 0))]
    enum MyInterface {
        InterfaceObj(InterfaceObj),
    }

    struct MyObj;

    #[Object(internal)]
    #[allow(unreachable_code)]
    impl MyObj {
        async fn a(&self) -> i32 {
            todo!()
        }

        async fn b(&self) -> i32 {
            todo!()
        }

        async fn c(&self) -> MyObj {
            todo!()
        }
    }

    #[Object(internal)]
    #[allow(unreachable_code)]
    impl Query {
        async fn value(&self) -> i32 {
            todo!()
        }

        async fn simple_obj(&self) -> MySimpleObj {
            todo!()
        }

        async fn interface_obj(&self) -> MyInterface {
            todo!()
        }

        async fn obj(&self) -> MyObj {
            todo!()
        }
    }

    struct MySubscription;

    #[Subscription(internal)]
    #[allow(unreachable_code)]
    impl MySubscription {
        #[graphql(depth_cost = 0)]
        async fn value(&self) -> BoxStream<'static, i32> {
            todo!()
        }
    }

    fn check_depth(query: &str, expect_depth: usize) {
        let registry =
            Schema::<Query, EmptyMutation, EmptySubscription>::create_registry(Default::default());
        let doc = parse_query(query).unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut depth = 0;
        let mut depth_calculate = DepthCalculate::new(&mut depth);
        visit(&mut depth_calculate, &mut ctx, &doc);
        assert_eq!(depth, expect_depth);
    }

    fn check_subscription_depth(query: &str, expect_depth: usize) {
        let registry =
            Schema::<Query, EmptyMutation, MySubscription>::create_registry(Default::default());
        let doc = parse_query(query).unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut depth = 0;
        let mut depth_calculate = DepthCalculate::new(&mut depth);
        visit(&mut depth_calculate, &mut ctx, &doc);
        assert_eq!(depth, expect_depth);
    }

    #[test]
    fn depth() {
        check_depth(
            r#"{
            value #1
        }"#,
            1,
        );

        check_depth(
            r#"
        {
            obj { #1
                a b #2
            }
        }"#,
            2,
        );

        check_depth(
            r#"
        {
            obj { # 1
                a b c { # 2
                    a b c { # 3
                        a b # 4
                    }
                }
            }
        }"#,
            4,
        );

        check_depth(
            r#"
        fragment A on MyObj {
            a b ... A2 #2
        }

        fragment A2 on MyObj {
            obj {
                a #3
            }
        }

        query {
            obj { # 1
                ... A
            }
        }"#,
            3,
        );

        check_depth(
            r#"
        {
            obj { # 1
                ... on MyObj {
                    a b #2
                    ... on MyObj {
                        obj {
                            a #3
                        }
                    }
                }
            }
        }"#,
            3,
        );
    }

    #[test]
    fn simple_object_depth_cost() {
        check_depth(
            r#"{
                simpleObj { cheap }
            }"#,
            1,
        );

        check_depth(
            r#"{
                simpleObj { expensive }
            }"#,
            3,
        );
    }

    #[test]
    fn interface_depth_cost() {
        check_depth(
            r#"{
                interfaceObj { value }
            }"#,
            1,
        );

        check_depth(
            r#"
            fragment ObjFields on InterfaceObj {
                specific
            }

            {
                interfaceObj { ...ObjFields }
            }"#,
            1,
        );
    }

    #[test]
    fn subscription_depth_cost() {
        check_subscription_depth(
            r#"subscription {
                value
            }"#,
            0,
        );
    }

    #[test]
    fn depth_cost_zero_ignores_field() {
        struct QueryWithZeroCost;
        struct ObjWithZeroCost;

        #[Object(internal)]
        #[allow(unreachable_code)]
        impl ObjWithZeroCost {
            #[graphql(depth_cost = 0)]
            async fn normal(&self) -> i32 {
                todo!()
            }

            async fn nested(&self) -> ObjWithZeroCost {
                todo!()
            }
        }

        #[Object(internal)]
        #[allow(unreachable_code)]
        impl QueryWithZeroCost {
            async fn value(&self) -> i32 {
                todo!()
            }

            async fn obj(&self) -> ObjWithZeroCost {
                todo!()
            }
        }

        let registry =
            Schema::<QueryWithZeroCost, EmptyMutation, EmptySubscription>::create_registry(
                Default::default(),
            );

        // Query: obj { normal normal normal }
        // obj costs 1, each normal costs 0, so depth = 1
        let doc = parse_query(
            r#"{
                obj {
                    normal
                    normal
                    normal
                }
            }"#,
        )
        .unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut depth = 0;
        let mut depth_calculate = DepthCalculate::new(&mut depth);
        visit(&mut depth_calculate, &mut ctx, &doc);
        assert_eq!(depth, 1);

        // Query: obj { nested { normal } }
        // obj costs 1, nested costs 1, normal costs 0, so depth = 2
        let doc = parse_query(
            r#"{
                obj {
                    nested {
                        normal
                    }
                }
            }"#,
        )
        .unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut depth = 0;
        let mut depth_calculate = DepthCalculate::new(&mut depth);
        visit(&mut depth_calculate, &mut ctx, &doc);
        assert_eq!(depth, 2);
    }

    #[test]
    fn depth_cost_custom_values() {
        // Test with custom depth cost values (e.g., 2 for expensive fields)
        struct QueryCustomCost;
        struct ObjCustomCost;

        #[Object(internal)]
        #[allow(unreachable_code)]
        impl ObjCustomCost {
            async fn cheap(&self) -> i32 {
                todo!()
            }

            #[graphql(depth_cost = 2)]
            async fn expensive(&self) -> ObjCustomCost {
                todo!()
            }
        }

        #[Object(internal)]
        #[allow(unreachable_code)]
        impl QueryCustomCost {
            async fn obj(&self) -> ObjCustomCost {
                todo!()
            }
        }

        let registry = Schema::<QueryCustomCost, EmptyMutation, EmptySubscription>::create_registry(
            Default::default(),
        );

        // Query: obj { expensive { cheap } }
        // obj costs 1, expensive costs 2, cheap costs 1, so max depth = 1 + 2 + 1 = 4
        let doc = parse_query(
            r#"{
                obj {
                    expensive {
                        cheap
                    }
                }
            }"#,
        )
        .unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut depth = 0;
        let mut depth_calculate = DepthCalculate::new(&mut depth);
        visit(&mut depth_calculate, &mut ctx, &doc);
        assert_eq!(depth, 4);
    }
}
