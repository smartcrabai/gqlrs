fn main() {
    // Expose the `no_send` feature flag to dependent crates via the
    // DEP_GQLRS_NO_SEND environment variable.  Integration crates
    // (poem, warp, actix-web, axum, rocket) use this to detect that
    // gqlrs was compiled without Send/Sync on futures and compile
    // themselves down to an empty crate, since the web frameworks
    // they wrap inherently require Send.
    #[cfg(feature = "no_send")]
    println!("cargo:no_send=true");
}
