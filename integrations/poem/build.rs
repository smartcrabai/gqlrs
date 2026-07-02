fn main() {
    println!("cargo::rustc-check-cfg=cfg(gqlrs_no_send)");
    if std::env::var("DEP_GQLRS_NO_SEND").is_ok() {
        println!("cargo::rustc-cfg=gqlrs_no_send");
    }
}
