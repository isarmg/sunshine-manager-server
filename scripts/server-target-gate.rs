pub fn main() {
    let target = std::env::var("TARGET").expect("Cargo must provide TARGET");
    assert_eq!(
        target, "x86_64-unknown-linux-gnu",
        "xscs server supports only x86_64-unknown-linux-gnu (Linux AMD64)"
    );
}
