//! Build script: stamps the build with the short git commit hash so
//! every NewNyth binary can show which revision it was built from
//! (see [`version_with_commit`](crate::version_with_commit) in the
//! crate root). Building from a tree without a git checkout (a source
//! tarball, say) yields an empty hash and the plain package version is
//! shown instead.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-env=GIT_HASH={}", short_head_hash());
    watch_git_head();
}

/// `git rev-parse --short HEAD`, empty when git is unavailable or the
/// build tree is not a checkout.
fn short_head_hash() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

/// Re-run the script whenever HEAD, or the branch ref it points at,
/// moves: a pull or checkout must refresh the stamp even when this
/// crate's own files are untouched. Paths that do not exist (packed
/// refs, git worktrees) make cargo run the script again on every
/// build, which stays correct because a git call is cheap and the
/// emitted hash itself rarely changes.
fn watch_git_head() {
    let git_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.git");
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    if let Ok(text) = std::fs::read_to_string(&head)
        && let Some(reference) = text.trim().strip_prefix("ref: ")
    {
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join(reference).display()
        );
    }
}
