//! Async-graphql integration with Warp
#![allow(clippy::type_complexity)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(not(gqlrs_no_send))]
mod batch_request;
#[cfg(not(gqlrs_no_send))]
mod error;
#[cfg(not(gqlrs_no_send))]
mod request;
#[cfg(not(gqlrs_no_send))]
mod subscription;

#[cfg(not(gqlrs_no_send))]
pub use batch_request::{GraphQLBatchResponse, graphql_batch, graphql_batch_opts};
#[cfg(not(gqlrs_no_send))]
pub use error::GraphQLBadRequest;
#[cfg(not(gqlrs_no_send))]
pub use request::{GraphQLResponse, graphql, graphql_opts};
#[cfg(not(gqlrs_no_send))]
pub use subscription::{GraphQLWebSocket, graphql_protocol, graphql_subscription};
