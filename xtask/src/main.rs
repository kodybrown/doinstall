//! Local development commands depend on Cargo alone, never on the installer
//! product or dotask. Keep live tooling separate from rebuilt release binaries.
use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
fn main() {
  if let Err(error) = run() {
    eprintln!("doinstall build: {error:#}");
    std::process::exit(1);
  }
}
fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
fn execute(command: &mut Command) -> Result<()> {
  let code = command.status().context("Cannot start development tool")?;
  ensure!(code.success(), "Development tool failed: {code}");
  Ok(())
}
fn cargo(arguments: &[&str]) -> Result<()> {
  let mut command = Command::new("cargo");
  command.current_dir(root()).args(arguments);
  if cfg!(windows) {
    command.env("RUSTFLAGS", "-C target-feature=+crt-static");
  }
  execute(&mut command)
}
fn target() -> Result<PathBuf> {
  let result = Command::new("cargo")
    .current_dir(root())
    .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
    .output()?;
  ensure!(
    result.status.success(),
    "Cargo metadata failed: {}",
    String::from_utf8_lossy(&result.stderr)
  );
  let metadata: serde_json::Value = serde_json::from_slice(&result.stdout)?;
  Ok(
    metadata["target_directory"]
      .as_str()
      .context("Missing target_directory")?
      .into(),
  )
}
fn run() -> Result<()> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  let Some(action) = args.first() else {
    println!("Commands: build [--stage-dir DIR], verify, shim [--verify]");
    return Ok(());
  };
  match action.as_str() {
    "build" => {
      let stage = match &args[1..] {
        [] => None,
        [option, path] if option == "--stage-dir" => Some(PathBuf::from(path)),
        _ => bail!("Use build [--stage-dir DIR]"),
      };
      cargo(&["build", "--package", "doinstall", "--locked", "--release"])?;
      let output = target()?.join("release");
      if let Some(stage) = stage {
        fs::create_dir_all(&stage)?;
        for name in ["doinstall", "doinstall-builder"] {
          let name = format!("{name}{}", if cfg!(windows) { ".exe" } else { "" });
          fs::copy(output.join(&name), stage.join(name))?;
        }
      }
      println!("{}", output.display());
      Ok(())
    }
    "verify" => {
      ensure!(args.len() == 1, "verify takes no arguments");
      cargo(&["fmt", "--all", "--", "--check"])?;
      cargo(&["test", "--package", "doinstall", "--locked", "--release"])?;
      cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
      ])?;
      shim(true)?;
      execute(Command::new("git").current_dir(root()).args(["diff", "--check"]))
    }
    "shim" => {
      ensure!(args.len() == 1 || args[1..] == ["--verify"], "Use shim [--verify]");
      shim(args.len() == 2)
    }
    _ => bail!("Unknown command: {action}"),
  }
}
fn shim(verify: bool) -> Result<()> {
  let source = root().join("src/shim");
  let assets = source.join("assets");
  if !verify {
    let stage = tempfile::tempdir()?;
    for (name, triple, machine) in [
      ("x64", "x86_64", "i386:x86-64"),
      ("arm64", "aarch64", "arm64"),
    ] {
      let library = stage.path().join(format!("kernel32-{name}.lib"));
      let object = stage.path().join(format!("shim-{name}.obj"));
      execute(
        Command::new("llvm-dlltool")
          .args(["-m", machine, "-d"])
          .arg(source.join("kernel32.def"))
          .arg("-l")
          .arg(&library),
      )?;
      execute(
        Command::new("clang")
          .arg(format!("--target={triple}-pc-windows-msvc"))
          .arg("-c")
          .arg(source.join("shim.c"))
          .arg("-o")
          .arg(&object)
          .args([
            "-Os",
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-Wall",
            "-Wextra",
            "-Werror",
          ]),
      )?;
      execute(
        Command::new("lld-link")
          .arg(object)
          .arg(library)
          .args([
            "/entry:mainCRTStartup",
            "/subsystem:console",
            "/nodefaultlib",
            "/dynamicbase",
            "/nxcompat",
            "/timestamp:0",
            "/Brepro",
          ])
          .arg(format!("/out:{}", assets.join(format!("win-{name}.exe")).display())),
      )?;
    }
  }
  let mut hashes = serde_json::Map::new();
  for name in [
    "shim.c",
    "kernel32.def",
    "assets/win-x64.exe",
    "assets/win-arm64.exe",
  ] {
    hashes.insert(
      name.into(),
      format!("{:x}", Sha256::digest(fs::read(source.join(name))?)).into(),
    );
  }
  hashes.insert(
    "build-tool.rs".into(),
    format!("{:x}", Sha256::digest(fs::read(root().join("xtask/src/main.rs"))?)).into(),
  );
  let text = serde_json::to_string_pretty(&hashes)? + "\n";
  if verify {
    ensure!(
      fs::read_to_string(assets.join("hashes.json"))? == text,
      "Shim sources, binaries, or build tool changed; rebuild and review with build shim"
    );
  } else {
    fs::write(assets.join("hashes.json"), text)?;
  }
  Ok(())
}
