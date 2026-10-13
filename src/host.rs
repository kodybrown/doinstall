//! Destination checks are separate from build-machine schema validation.
use crate::{
  config,
  model::{HostRequirements, Package},
};
use anyhow::{ensure, Context, Result};
use std::collections::BTreeMap;

pub(crate) fn validate(package: &Package) -> Result<()> {
  ensure!(
    package.platform == config::platform() && package.architecture == config::architecture(),
    "Package targets {}-{}; engine is {}-{}",
    package.platform,
    package.architecture,
    config::platform(),
    config::architecture()
  );
  if let Some(host) = &package.runtime.host {
    ensure!(cfg!(target_os = "linux"), "Distribution-gated installation requires Linux.");
    let release = std::fs::read_to_string("/etc/os-release").context("Read /etc/os-release before installing")?;
    distribution(host, &release)?;
    if host.require_root {
      #[cfg(unix)]
      ensure!(unsafe { libc::geteuid() } == 0, "Installer requires root.");
      #[cfg(not(unix))]
      anyhow::bail!("Installer requires Linux root.");
    }
    if host.require_systemd {
      ensure!(
        std::path::Path::new("/run/systemd/system").is_dir(),
        "Installer requires a running systemd host."
      );
    }
  }
  Ok(())
}
fn distribution(host: &HostRequirements, text: &str) -> Result<()> {
  let mut fields = BTreeMap::new();
  for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
    if let Some((name, value)) = line.split_once('=') {
      let value = value.trim().trim_matches('"').trim_matches('\'');
      ensure!(fields.insert(name.trim(), value).is_none(), "Duplicate os-release field.");
    }
  }
  let id = fields.get("ID").context("os-release requires ID")?;
  let version = fields.get("VERSION_ID").context("os-release requires VERSION_ID")?;
  ensure!(
    host
      .distributions
      .get(*id)
      .is_some_and(|versions| versions.iter().any(|v| v == version)),
    "Unsupported destination OS: {id} {version}."
  );
  Ok(())
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn distribution_gate_does_not_admit_derivatives_or_unknown_versions() {
    let host = HostRequirements {
      distributions: BTreeMap::from([
        ("debian".into(), vec!["12".into()]),
        ("ubuntu".into(), vec!["24.04".into()]),
      ]),
      ..Default::default()
    };
    assert!(distribution(&host, "ID=debian\nVERSION_ID=\"12\"\n").is_ok());
    assert!(distribution(&host, "ID=ubuntu\nVERSION_ID='24.04'\n").is_ok());
    assert!(distribution(&host, "ID=mint\nID_LIKE=ubuntu\nVERSION_ID=24.04\n").is_err());
    assert!(distribution(&host, "ID=debian\nVERSION_ID=99\n").is_err());
    assert!(distribution(&host, "ID=debian\nID=ubuntu\nVERSION_ID=12\n").is_err());
  }
}
