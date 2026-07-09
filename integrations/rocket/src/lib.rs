//! Async-graphql integration with Rocket.
//!
//! Note: This integrates with the unreleased version 0.5 of Rocket, and so
//! breaking changes in both this library and Rocket are to be expected.
//!
//! To configure options for sending and receiving multipart requests, add your
//! instance of `MultipartOptions` to the state managed by Rocket
//! (`.manage(your_multipart_options)`).
//!
//! **[Full Example](<https://github.com/async-graphql/examples/blob/master/rocket/starwars/src/main.rs>)**

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![allow(clippy::blocks_in_conditions)]

// When gqlrs is compiled with `no_send`, the integration crate is empty
// because web frameworks inherently require Send on futures.
#[cfg(not(gqlrs_no_send))]
include!("rocket_impl.rs");
