use std::process::Command;

use chrono::Local;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=.git/packed-refs");

    let build_date = Local::now().to_rfc3339();
    println!("cargo:rustc-env=CODEX_SWITCH_BUILD_DATE={build_date}");

    if let Some(git_commit) = git_output(&["rev-parse", "--short=12", "HEAD"]) {
        println!("cargo:rustc-env=CODEX_SWITCH_GIT_COMMIT={git_commit}");
    }

    let git_ref = git_output(&["describe", "--tags", "--exact-match"])
        .or_else(|| git_output(&["branch", "--show-current"]))
        .or_else(|| git_output(&["rev-parse", "--short=12", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CODEX_SWITCH_GIT_REF={git_ref}");
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}