//! Async-graphql integration with Axum
#![forbid(unsafe_code)]
#![allow(clippy::uninlined_format_args)]
#![warn(missing_docs)]

#[cfg(not(gqlrs_no_send))]
mod extract;
#[cfg(not(gqlrs_no_send))]
mod query;
#[cfg(not(gqlrs_no_send))]
mod response;
#[cfg(not(any(gqlrs_no_send, target_arch = "wasm32")))]
mod subscription;

#[cfg(not(gqlrs_no_send))]
pub use extract::{GraphQLBatchRequest, GraphQLRequest, rejection};
#[cfg(not(gqlrs_no_send))]
pub use query::GraphQL;
#[cfg(not(gqlrs_no_send))]
pub use response::GraphQLResponse;
#[cfg(not(any(gqlrs_no_send, target_arch = "wasm32")))]
pub use subscription::{GraphQLProtocol, GraphQLSubscription, GraphQLWebSocket};
