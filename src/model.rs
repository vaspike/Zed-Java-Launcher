use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    SpringBoot,
    Main,
    TestClass,
    TestMethod,
}

impl EntryKind {
    pub fn is_test(&self) -> bool {
        matches!(self, Self::TestClass | Self::TestMethod)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub kind: EntryKind,
    pub module: String,
    pub project_name: String,
    pub class: String,
    pub method: Option<String>,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub path: String,
    pub artifact_id: String,
    pub packaging: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub root: PathBuf,
    pub modules: Vec<Module>,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}

impl Project {
    pub fn entry(&self, query: &str) -> Result<&Entry> {
        if let Some(entry) = self.entries.iter().find(|e| e.id == query) {
            return Ok(entry);
        }
        let matches: Vec<_> = self
            .entries
            .iter()
            .filter(|e| {
                e.class == query
                    || (e.module == query && !e.kind.is_test())
                    || e.class.rsplit('.').next() == Some(query)
            })
            .collect();
        match matches.as_slice() {
            [entry] => Ok(entry),
            [] => bail!("Entry not found: {query}. Use `scan` to list stable IDs."),
            _ => bail!(
                "Ambiguous entry {query}; use an ID:\n{}",
                matches
                    .iter()
                    .map(|e| e.id.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaunchOptions {
    pub spring_profile: Option<String>,
    pub vm_args: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub env: BTreeMap<String, String>,
    pub env_file: Option<String>,
    pub cwd: Option<String>,
    pub java_home: Option<String>,
    pub maven_profiles: Option<Vec<String>>,
    pub maven_args: Option<Vec<String>>,
    pub build: Option<bool>,
    // JDTLS project names need not equal Maven artifactIds.
    pub debug_project_name: Option<String>,
    /// When true, automatically terminate this service if the parent Zed process exits.
    pub terminate_on_zed_quit: Option<bool>,
}

impl LaunchOptions {
    pub fn overlay(&self, other: &Self) -> Self {
        let mut env = self.env.clone();
        env.extend(other.env.clone());
        Self {
            spring_profile: other.spring_profile.clone().or(self.spring_profile.clone()),
            vm_args: other.vm_args.clone().or(self.vm_args.clone()),
            args: other.args.clone().or(self.args.clone()),
            env,
            env_file: other.env_file.clone().or(self.env_file.clone()),
            cwd: other.cwd.clone().or(self.cwd.clone()),
            java_home: other.java_home.clone().or(self.java_home.clone()),
            maven_profiles: other.maven_profiles.clone().or(self.maven_profiles.clone()),
            maven_args: other.maven_args.clone().or(self.maven_args.clone()),
            build: other.build.or(self.build),
            debug_project_name: other
                .debug_project_name
                .clone()
                .or(self.debug_project_name.clone()),
            terminate_on_zed_quit: other
                .terminate_on_zed_quit
                .or(self.terminate_on_zed_quit),
        }
    }
    pub fn jvm_args(&self) -> Vec<String> {
        let mut args = self.vm_args.clone().unwrap_or_default();
        if let Some(profile) = &self.spring_profile {
            args.retain(|a| !a.starts_with("-Dspring.profiles.active="));
            if !profile.is_empty() {
                args.push(format!("-Dspring.profiles.active={profile}"));
            }
        }
        args
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupItem {
    pub entry: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub version: u32,
    pub defaults: LaunchOptions,
    pub entries: BTreeMap<String, LaunchOptions>,
    pub groups: BTreeMap<String, Vec<GroupItem>>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema: Some("./config.schema.json".into()),
            version: 1,
            defaults: LaunchOptions::default(),
            entries: BTreeMap::new(),
            groups: BTreeMap::new(),
        }
    }
}
impl Config {
    pub fn options(&self, id: &str) -> LaunchOptions {
        self.defaults
            .overlay(self.entries.get(id).unwrap_or(&LaunchOptions::default()))
    }
}
