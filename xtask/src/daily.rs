//! Local tasks share the Cargo-only helper so profile selection, inspection and
//! literal argument forwarding behave the same with or without installed dotask.
use anyhow::{bail, ensure, Context, Result};
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug)]
struct Options {
  profile: String,
  stage: Option<PathBuf>,
  bin: String,
  arguments: Vec<String>,
}
impl Options {
  fn parse(action: &str, arguments: &[String]) -> Result<Self> {
    let mut value = Self {
      profile: "dev".into(),
      stage: None,
      bin: "doinstall".into(),
      arguments: vec![],
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut items = arguments.iter();
    while let Some(option) = items.next() {
      if option == "--" && action == "run" {
        ensure!(!seen.contains("--args"), "Choose --args or trailing arguments");
        value.arguments = items.cloned().collect();
        break;
      }
      ensure!(seen.insert(option.as_str()), "Duplicate option: {option}");
      match option.as_str() {
        "--profile" => {
          value.profile = items.next().context("--profile requires dev or release")?.clone();
          ensure!(
            ["dev", "release"].contains(&value.profile.as_str()),
            "Use profile dev or release"
          );
        }
        "--stage-dir" if action == "build" => {
          value.stage = Some(items.next().context("--stage-dir requires a directory")?.into());
        }
        "--bin" if action == "run" => {
          value.bin = items.next().context("--bin requires a tool")?.clone();
          ensure!(super::release::TOOLS.contains(&value.bin.as_str()), "Unknown doinstall tool");
        }
        "--args" if action == "run" => {
          value.arguments = serde_json::from_str(items.next().context("--args requires JSON")?)
            .context("--args must be a JSON array of strings")?;
        }
        _ => bail!("Unexpected {action} option: {option}"),
      }
    }
    Ok(value)
  }
}
fn cargo(arguments: &[&str]) -> Result<()> {
  let mut command = Command::new("cargo");
  command.current_dir(super::root()).args(arguments);
  // Keep lint/test/build flags identical; differing flags invalidate Cargo's cache.
  super::release::configure(&mut command)?;
  super::execute(&mut command)
}
fn lint(profile: &str) -> Result<()> {
  cargo(&[
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--profile",
    profile,
    "--",
    "-D",
    "warnings",
  ])
}
pub fn run(action: &str, arguments: &[String]) -> Result<()> {
  let options = Options::parse(action, arguments)?;
  // Refuse source-local Cargo outputs before lint or tests can compile anything.
  super::release::check_output()?;
  if action == "verify" {
    // Tests rebuild the workspace helper. Launchers use the separate bootstrap
    // profile so Windows never needs to replace a running test-output binary.
    let test_output = super::target()?.join(if options.profile == "dev" {
      "debug"
    } else {
      "release"
    });
    ensure!(
      !fs::canonicalize(std::env::current_exe()?)?.starts_with(super::release::destination(&test_output)?),
      "Run verification through build.cmd/build.sh, or cargo run --profile bootstrap --package doinstall-dev -- verify"
    );
  }
  println!(
    "Development checkout: {}; profile: {}",
    super::root().display(),
    options.profile
  );
  if action == "verify" {
    cargo(&["fmt", "--all", "--", "--check"])?;
  } else {
    cargo(&["fmt", "--all"])?;
    lint(&options.profile)?;
  }
  let output = super::release::build_profile(&options.profile)?;
  match action {
    "verify" => {
      cargo(&[
        "test",
        "--workspace",
        "--locked",
        "--profile",
        &options.profile,
      ])?;
      lint(&options.profile)?;
      // Test feature unification may replace ordinary binaries. Inspect final bytes.
      super::release::scan_tools(&options.profile)?;
      super::shim(true)?;
      super::execute(Command::new("git").current_dir(super::root()).args(["diff", "--check"]))
    }
    "run" => {
      super::execute(Command::new(output.join(super::installer::executable(&options.bin))).args(&options.arguments))
    }
    "build" => {
      if let Some(stage) = options.stage {
        ensure!(!stage.as_os_str().is_empty(), "Stage directory must not be empty");
        let stage = super::release::destination(&stage)?;
        let root = fs::canonicalize(super::root())?;
        ensure!(
          !stage.starts_with(&root) && !root.starts_with(&stage),
          "Stage output must stay outside source"
        );
        fs::create_dir_all(&stage)?;
        for name in super::release::TOOLS {
          let name = super::installer::executable(name);
          if stage.join(&name) != fs::canonicalize(output.join(&name))? {
            fs::copy(output.join(&name), stage.join(name))?;
          }
        }
      }
      println!("{}", output.display());
      Ok(())
    }
    _ => bail!("Unknown development command"),
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  fn parse(action: &str, args: &[&str]) -> Result<Options> {
    Options::parse(action, &args.iter().map(|v| (*v).into()).collect::<Vec<_>>())
  }
  #[test]
  fn profiles_and_staging_are_explicit_and_validated_before_building() {
    assert_eq!(parse("build", &[]).unwrap().profile, "dev");
    assert_eq!(parse("verify", &["--profile", "release"]).unwrap().profile, "release");
    assert_eq!(
      parse("build", &["--stage-dir", "tools with spaces"]).unwrap().stage.unwrap(),
      PathBuf::from("tools with spaces")
    );
    for args in [
      vec!["--profile"],
      vec!["--profile", "../escape"],
      vec!["--profile", "dev", "--profile", "release"],
    ] {
      assert!(parse("build", &args).is_err());
    }
    assert!(parse("verify", &["--stage-dir", "tools"]).is_err());
  }
  #[test]
  fn run_preserves_literal_arguments_and_refuses_ambiguous_input() {
    let expected = vec!["", "a b", "日本語", "$(literal)"];
    assert_eq!(
      parse("run", &["--args", r#"["","a b","日本語","$(literal)"]"#])
        .unwrap()
        .arguments,
      expected
    );
    assert_eq!(
      parse("run", &["--", "", "a b", "日本語", "$(literal)"]).unwrap().arguments,
      expected
    );
    assert!(parse("run", &["--args", "[]", "--", "x"]).is_err());
    assert!(parse("run", &["--args", "[1]"]).is_err());
    assert!(parse("run", &["--bin", "../other"]).is_err());
  }
}
