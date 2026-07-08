# 订阅

订阅根对象和其它根对象定义稍有不同，它的 Resolver 函数总是返回一个 [Stream](https://docs.rs/futures-core/~0.3/futures_core/stream/trait.Stream.html)，或者返回成功值为流且外层类型名为 `Result` / `FieldResult` 的结果类型，而字段参数通常作为数据的筛选条件。

下面的例子订阅一个整数流，它每秒产生一个整数，参数`step`指定了整数的步长，默认为 1。

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

订阅 Resolver 也可以在创建流之前失败。这时请直接返回 `Result<impl Stream<Item = T>>`（或 `FieldResult<impl Stream<Item = T>>`）。`#[Subscription]` 宏会根据外层结果包装类型的名称 `Result` 或 `FieldResult` 来识别它，包括 `async_graphql::Result` 这样的限定路径；`MyResult<impl Stream<Item = T>>` 等其它别名不会被当作可失败的订阅 Resolver。

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
