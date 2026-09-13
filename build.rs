use std::{collections::BTreeSet, env, fs};

const FOUNDATION_SOURCE: &str = "git+https://github.com/isarmg/sarmg-foundation-server.git?rev=";

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
        "Cargo.lock contains no sarmg-foundation-server dependency"
    );
    assert_eq!(
        revisions.len(),
        1,
        "all sarmg-foundation-server packages must use one locked revision"
    );
    revisions.into_iter().next().expect("checked non-empty")
}

fn main() {
    let target = env::var("TARGET").expect("Cargo must provide TARGET");
    assert_eq!(
        target, "x86_64-unknown-linux-gnu",
        "Sunshine Manager server supports only x86_64-unknown-linux-gnu (Linux AMD64)"
    );
    let source_revision =
        env::var("SUNSHINE_MANAGER_SOURCE_REVISION").unwrap_or_else(|_| "unbound".to_owned());
    assert!(
        source_revision == "unbound"
            || (source_revision.len() == 40
                && source_revision
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))),
        "SUNSHINE_MANAGER_SOURCE_REVISION must be a full lowercase 40-hex Git commit"
    );
    println!("cargo:rustc-env=SUNSHINE_MANAGER_SOURCE_REVISION={source_revision}");
    let lockfile = fs::read_to_string("Cargo.lock").expect("read Cargo.lock");
    let foundation_revision = locked_foundation_revision(&lockfile);
    println!("cargo:rustc-env=SARMG_FOUNDATION_REVISION={foundation_revision}");
    println!("cargo:rerun-if-env-changed=SUNSHINE_MANAGER_SOURCE_REVISION");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=release.json");
}
