// https://nx.dev/reference/nx-json

use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::PathBuf;

/// Only fields that are compatible with moon are documented,
/// anything else is ignored!

#[derive(Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum StringOrList {
    List(Vec<String>),
    String(String),
}

#[derive(Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum NxDependsOn {
    Object {
        dependencies: Option<bool>,
        target: String,
        params: Option<String>,
        projects: Option<StringOrList>,
    },
    String(String),
}

#[derive(Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum NxInput {
    DepOutput {
        dependent_tasks_output_files: String,
        transitive: Option<bool>,
    },
    External {
        external_dependencies: Vec<String>,
    },
    Env {
        env: String,
    },
    Runtime {
        runtime: String,
    },
    Source(String),
}

pub type NxNamedInputs = FxHashMap<String, Vec<NxInput>>;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxAffected {
    pub default_base: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxTargetOptions {
    pub cache: Option<bool>,
    pub depends_on: Option<Vec<String>>,
    pub executor: Option<String>,
    pub inputs: Option<Vec<NxInput>>,
    pub named_inputs: Option<NxNamedInputs>,
    pub outputs: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxJson {
    pub affected: Option<NxAffected>,
    pub cache_directory: Option<PathBuf>,
    pub capture_stderr: Option<bool>,
    pub encryption_key: Option<String>,
    pub named_inputs: Option<NxNamedInputs>,
    pub parallel: Option<usize>,
    pub selectively_hash_ts_config: Option<bool>,
    pub skip_nx_cache: Option<bool>,
    pub target_defaults: Option<FxHashMap<String, NxTargetOptions>>,
    // To support in the future
    // release
}
