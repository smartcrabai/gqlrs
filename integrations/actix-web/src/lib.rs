//! Async-graphql integration with Actix-web
#![forbid(unsafe_code)]
#![allow(clippy::upper_case_acronyms)]
#![warn(missing_docs)]

#[cfg(not(gqlrs_no_send))]
mod handler;
#[cfg(not(gqlrs_no_send))]
mod request;
#[cfg(not(gqlrs_no_send))]
mod subscription;

#[cfg(not(gqlrs_no_send))]
pub use handler::GraphQL;
#[cfg(not(gqlrs_no_send))]
pub use request::{GraphQLBatchRequest, GraphQLRequest, GraphQLResponse};
#[cfg(not(gqlrs_no_send))]
pub use subscription::GraphQLSubscription;
