#![cfg(feature = "no_send")]

use std::{cell::RefCell, rc::Rc};

use gqlrs::*;

#[derive(Clone)]
struct LocalState(Rc<RefCell<i32>>);

struct Query;

#[Object]
impl Query {
    async fn value(&self, ctx: &Context<'_>) -> i32 {
        let state = ctx.data_unchecked::<LocalState>();
        *state.0.borrow_mut() += 1;
        *state.0.borrow()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn supports_non_send_schema_data() {
    let state = LocalState(Rc::new(RefCell::new(0)));
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(state)
        .finish();

    let response = schema.execute("{ value }").await.into_result().unwrap();
    assert_eq!(response.data, value!({ "value": 1 }));
}
