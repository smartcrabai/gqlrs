# Subscription

The definition of the subscription root object is slightly different from other root objects. Its resolver function always returns a [Stream](https://docs.rs/futures-core/~0.3/futures_core/stream/trait.Stream.html), or an outer `Result` / `FieldResult` whose successful value is a stream, and the field parameters are usually used as data filtering conditions.

The following example subscribes to an integer stream, which generates one integer per second. The parameter `step` specifies the integer step size with a default of 1.

```rust
# extern crate async_graphql;
# use std::time::Duration;
# use async_graphql::futures_util::stream::Stream;
# use async_graphql::futures_util::StreamExt;
# extern crate tokio_stream;
# extern crate tokio;
use async_graphql::*;

struct Subscription;

#[Subscription]
impl Subscription {
    async fn integers(&self, #[graphql(default = 1)] step: i32) -> impl Stream<Item = i32> {
        let mut value = 0;
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
            .map(move |_| {
                value += step;
                value
            })
    }
}
```

A subscription resolver can fail before it creates the stream. In that case, return `Result<impl Stream<Item = T>>` (or `FieldResult<impl Stream<Item = T>>`) directly. The `#[Subscription]` macro recognizes the outer result wrapper by the type name `Result` or `FieldResult`, including qualified paths like `async_graphql::Result`; other aliases such as `MyResult<impl Stream<Item = T>>` are not treated as fallible subscription resolvers.

```rust
# extern crate async_graphql;
# use async_graphql::futures_util::stream::{self, Stream};
use async_graphql::*;

struct FallibleSubscription;

#[Subscription]
impl FallibleSubscription {
    async fn checked_integers(&self, enabled: bool) -> Result<impl Stream<Item = i32>> {
        if !enabled {
            return Err("subscription is disabled".into());
        }

        Ok(stream::iter([1, 2, 3]))
    }
}
```
