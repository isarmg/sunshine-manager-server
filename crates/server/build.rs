#[path = "../../scripts/server-target-gate.rs"]
mod target_gate;
use std::{collections::BTreeSet, env, fs};

const FOUNDATION_SOURCE: &str = "git+https://github.com/isarmg/xcss.git?rev=";

fn locked_foundation_revision(lockfile: &str) -> String {
    let mut revisions = BTreeSet::new();
    for line in lockfile.lines().map(str::trim) {
        let Some(source) = line
            .strip_prefix("source = \"")
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        let Some(rest) = source.strip_prefix(FOUNDATION_SOURCE) else {
            continue;
        };
        let (requested, locked) = rest
            .split_once('#')
            .expect("Foundation Cargo.lock source must contain a locked commit");
        assert!(
            requested == locked
                && locked.len() == 40
                && locked
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
            "Foundation Cargo.lock source must bind one exact lowercase 40-hex revision"
        );
        revisions.insert(locked.to_owned());
    }
    assert!(
        !revisions.is_empty(),
        "Cargo.lock contains no xcss dependency"
    );
    assert_eq!(
        revisions.len(),
        1,
        "all xcss packages must use one locked revision"
    );
    revisions.into_iter().next().expect("checked non-empty")
}

fn main() {
    target_gate::main();
    let product_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("server crate belongs to the product workspace");
    let web_root = env::var_os("XCSS_WEB_DIST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| product_root.join("web/dist"));
    xcss_web_assets::build::generate(&web_root).expect("generate embedded Web assets");
    println!("cargo:rerun-if-env-changed=XCSS_WEB_DIST");
    let source_revision = env::var("XSCS_SOURCE_REVISION").unwrap_or_else(|_| "unbound".to_owned());
    assert!(
        source_revision == "unbound"
            || (source_revision.len() == 40
                && source_revision
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))),
        "XSCS_SOURCE_REVISION must be a full lowercase 40-hex Git commit"
    );
    println!("cargo:rustc-env=XSCS_SOURCE_REVISION={source_revision}");
    let lockfile = fs::read_to_string(product_root.join("Cargo.lock")).expect("read Cargo.lock");
    let foundation_revision = locked_foundation_revision(&lockfile);
    println!("cargo:rustc-env=XCSS_FOUNDATION_REVISION={foundation_revision}");
    println!("cargo:rerun-if-env-changed=XSCS_SOURCE_REVISION");
    println!(
        "cargo:rerun-if-changed={}",
        product_root.join("Cargo.lock").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        product_root.join("release.json").display()
    );
}
