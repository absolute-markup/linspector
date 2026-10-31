//! Real helper-process rejection tests. No elevation and no native mutations.
use std::process::{Command, Stdio};
#[test]
fn helper_rejects_arbitrary_command_arguments_before_any_operation() {
    let output = Command::new(env!("CARGO_BIN_EXE_linspector-helper"))
        .args(["/usr/bin/sh", "-c", "false"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(40));
    assert!(String::from_utf8_lossy(&output.stderr).contains("no command-line arguments"));
}
#[test]
fn ordinary_helper_launch_without_polkit_caller_cannot_change_anything() {
    let output = Command::new(env!("CARGO_BIN_EXE_linspector-helper"))
        .env_remove("PKEXEC_UID")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(40));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("requires root authentication") || error.contains("Missing polkit caller"),
        "{error}"
    );
}
