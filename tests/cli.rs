use std::process::Command;

#[test]
fn version_and_help_work_without_a_package() {
  let directory = tempfile::tempdir().unwrap();
  let run = |argument| {
    let output = Command::new(env!("CARGO_BIN_EXE_doinstall"))
      .current_dir(directory.path())
      .arg(argument)
      .output()
      .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
  };
  let version = run("--version");
  let identity = version.trim_end();
  assert!(identity.starts_with(&format!("doinstall v{}+", env!("CARGO_PKG_VERSION"))));
  assert_eq!(version.lines().count(), 1);
  assert!(!identity.contains("(built"));
  for argument in ["--help", "-h"] {
    let help = run(argument);
    assert_eq!(help.lines().next(), Some(identity));
    assert!(help.contains("--version                    Show version and exit"));
    assert!(help.contains("--config FILE"));
  }
  assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
