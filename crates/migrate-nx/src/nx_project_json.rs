// https://nx.dev/reference/project-configuration

use crate::nx_json::{NxNamedInputs, NxTargetOptions};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxProjectJson {
    pub implicit_dependencies: Option<Vec<String>>,
    pub name: Option<String>,
    pub named_inputs: Option<NxNamedInputs>,
    pub project_type: Option<String>,
    pub root: Option<PathBuf>,
    pub source_root: Option<PathBuf>,
    pub tags: Option<Vec<String>>,
    pub targets: Option<FxHashMap<String, NxTargetOptions>>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageJsonWithNx {
    pub nx: Option<NxProjectJson>,
}
