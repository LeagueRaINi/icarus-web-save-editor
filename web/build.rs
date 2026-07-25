use std::process::Command;

/// Exposes the short git commit hash as GIT_HASH at compile time, when the
/// project is a git checkout. The footer falls back to the crate version
/// otherwise.
fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty());
    if let Some(h) = hash {
        println!("cargo:rustc-env=GIT_HASH={h}");
    }
    println!("cargo:rerun-if-changed=../.git/HEAD");
}
