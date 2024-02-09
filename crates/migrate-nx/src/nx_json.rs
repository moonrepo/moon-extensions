// https://nx.dev/reference/nx-json
// https://github.com/nrwl/nx/blob/master/packages/nx/schemas/nx-schema.json

use rustc_hash::FxHashMap;
use serde::Deserialize;

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
    Dep {
        dependencies: Option<bool>,
        projects: Option<StringOrList>,
        input: String,
    },
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
    Fileset {
        fileset: String, // path, glob
    },
    Runtime {
        runtime: String, // command line
    },
    Source(String), // path, glob, group reference
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
    pub command: Option<String>,
    pub depends_on: Option<Vec<NxDependsOn>>,
    pub default_configuration: Option<String>,
    pub executor: Option<String>,
    pub inputs: Option<Vec<NxInput>>,
    pub named_inputs: Option<NxNamedInputs>,
    pub outputs: Option<Vec<String>>,
    // Not supported:
    // options, configurations
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxJson {
    pub affected: Option<NxAffected>,
    pub named_inputs: Option<NxNamedInputs>,
    pub target_defaults: Option<FxHashMap<String, NxTargetOptions>>,
    // Not supported:
    // implicitDependencies, tasksRunnerOptions, workspaceLayout, generators,
    // plugins, defaultProject, nxCloud*, parallel, cacheDirectory, useDaemonProcess,
    // release,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceJson {
    pub projects: FxHashMap<String, String>,
}
