use std::process::Command;

#[test]
fn packaged_mcp_metadata_matches_the_crate_and_defaults_to_read_only() {
    let metadata: serde_json::Value =
        serde_json::from_str(include_str!("../packaging/mcp/server.json")).unwrap();

    assert_eq!(metadata["name"], "apple-mail");
    assert_eq!(metadata["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(metadata["transport"]["type"], "stdio");
    assert_eq!(metadata["transport"]["command"], "apple-mail");
    assert_eq!(metadata["transport"]["args"], serde_json::json!(["mcp"]));
    assert_eq!(metadata["default_policy"], "read-only");
}

#[test]
fn binary_reports_the_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_apple-mail"))
        .arg("--version")
        .output()
        .expect("version command should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("apple-mail {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
