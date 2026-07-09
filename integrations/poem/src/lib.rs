//! Async-graphql integration with Poem
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(not(gqlrs_no_send))]
mod extractor;
#[cfg(not(gqlrs_no_send))]
mod query;
#[cfg(not(gqlrs_no_send))]
mod response;
#[cfg(not(gqlrs_no_send))]
mod subscription;

#[cfg(not(gqlrs_no_send))]
pub use extractor::{GraphQLBatchRequest, GraphQLRequest};
#[cfg(not(gqlrs_no_send))]
pub use query::GraphQL;
#[cfg(not(gqlrs_no_send))]
pub use response::{GraphQLBatchResponse, GraphQLResponse};
#[cfg(not(gqlrs_no_send))]
pub use subscription::{GraphQLProtocol, GraphQLSubscription, GraphQLWebSocket};
