use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Package {
  pub schema: u32,
  pub application: Application,
  pub platform: String,
  pub architecture: String,
  pub payload: String,
  pub runtime: Runtime,
}

/// Options used on the destination machine. The creator preserves this layout
/// when writing the package. Producer-only packaging settings are omitted from
/// distributed YAML; destination runtime settings share the same schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Runtime {
  /// A theme identifier, never inline colors or layout settings.
  #[serde(default, skip_serializing_if = "ThemeName::is_default")]
  pub theme: ThemeName,
  #[serde(default = "interactive_default")]
  pub interactive: bool,
  // Omit the default policy so adding the option does not rewrite existing
  // package snapshots or change their content identity when it is unused.
  #[serde(default = "interactive_default", skip_serializing_if = "cancel_confirmation_default")]
  pub confirm_cancel: bool,
  #[serde(default)]
  pub inputs: BTreeMap<String, Input>,
  #[serde(default)]
  pub values: BTreeMap<String, serde_json::Value>,
  #[serde(default)]
  pub defaults: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
  #[serde(default)]
  pub profiles: BTreeMap<String, Profile>,
  #[serde(default)]
  pub commands: Vec<Command>,
  #[serde(default)]
  pub shortcuts: Vec<Shortcut>,
  #[serde(default)]
  pub settings: Vec<String>,
  /// Initialize mutable application settings outside immutable builds. Existing
  /// files remain user-owned unless an earlier installation recorded them.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub settings_files: Vec<SettingsFile>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub host: Option<HostRequirements>,
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub tools: BTreeMap<String, IncludedTool>,
  #[serde(default, skip_serializing_if = "Lifecycle::is_empty")]
  pub lifecycle: Lifecycle,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub managed_files: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeName {
  #[default]
  Default,
}
impl ThemeName {
  fn is_default(&self) -> bool {
    matches!(self, Self::Default)
  }
}

fn cancel_confirmation_default(value: &bool) -> bool {
  *value
}

fn interactive_default() -> bool {
  true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
  pub id: String,
  pub name: String,
  pub version: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub build: Option<Build>,
  #[serde(default)]
  pub author: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub copyright: String,
  #[serde(default)]
  pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  pub stamp: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub commit: Option<String>,
  #[serde(default)]
  pub dirty: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
  /// Literal application templates must survive installer input resolution.
  #[serde(default = "interactive_default", skip_serializing_if = "cancel_confirmation_default")]
  pub expand: bool,
  /// Equal indices share a visual group; this does not affect value resolution.
  #[serde(default, rename = "group-index", skip_serializing_if = "Option::is_none")]
  pub group_index: Option<u32>,
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(default)]
  pub required: bool,
  #[serde(default)]
  pub prompt: Option<String>,
  #[serde(default)]
  pub default: Option<serde_json::Value>,
  #[serde(default)]
  pub choices: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct SettingsFile {
  pub source: String,
  pub destination: String,
  pub format: SettingsFormat,
  pub if_exists: ExistingSettings,
  /// Existing JSON pointers in the defaults document mapped to input names.
  #[serde(default)]
  pub set: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsFormat {
  Yaml,
  Json,
  Text,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExistingSettings {
  Preserve,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
  #[serde(default)]
  pub detect: Vec<String>,
  pub values: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
  pub name: String,
  pub executable: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Shortcut {
  pub name: String,
  pub executable: String,
  #[serde(default)]
  pub arguments: Vec<String>,
  #[serde(default)]
  pub icon: Option<String>,
  #[serde(default)]
  pub working_directory: Option<String>,
  #[serde(default)]
  pub terminal: bool,
  #[serde(default)]
  pub desktop: bool,
  #[serde(default)]
  pub start_menu: bool,
  #[serde(default)]
  pub local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
  pub sha256: String,
  pub mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedFile {
  pub path: String,
  pub hash: Option<String>,
  pub link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BuildReceipt {
  pub schema: u32,
  pub app_id: String,
  pub build: String,
  pub files: BTreeMap<String, FileRecord>,
  pub config_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Receipt {
  pub schema: u32,
  pub app_id: String,
  pub root: String,
  pub active: String,
  pub previous: Option<String>,
  pub builds: BTreeMap<String, String>,
  pub launchers: Vec<OwnedFile>,
  #[serde(default)]
  pub shortcut_directories: Vec<String>,
  pub settings: Vec<String>,
  pub values: BTreeMap<String, serde_json::Value>,
  pub engine_hash: String,
  pub config_hash: String,
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub managed_files: BTreeMap<String, ManagedFile>,
  #[serde(default)]
  pub uninstalling: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct HostRequirements {
  #[serde(default)]
  pub distributions: BTreeMap<String, Vec<String>>,
  #[serde(default)]
  pub require_root: bool,
  #[serde(default)]
  pub require_systemd: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct IncludedTool {
  pub executable: String,
  pub sha256: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Lifecycle {
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub prepare: Vec<ToolCall>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub before_activate: Vec<ToolCall>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub after_activate: Vec<ToolCall>,
}
impl Lifecycle {
  pub fn is_empty(&self) -> bool {
    self.prepare.is_empty() && self.before_activate.is_empty() && self.after_activate.is_empty()
  }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ToolCall {
  pub tool: String,
  #[serde(default)]
  pub arguments: Vec<String>,
  #[serde(default)]
  pub environment: BTreeMap<String, String>,
  #[serde(default = "tool_timeout")]
  pub timeout_seconds: u64,
}
fn tool_timeout() -> u64 {
  600
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagedFile {
  pub hash: Option<String>,
  pub mode: u32,
}
