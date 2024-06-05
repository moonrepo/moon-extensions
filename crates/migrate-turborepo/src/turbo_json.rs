use rustc_hash::FxHashMap;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TurboOutputMode {
    #[default]
    Full,
    HashOnly,
    NewOnly,
    ErrorsOnly,
    None,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboTask {
    pub cache: Option<bool>,
    pub depends_on: Option<Vec<String>>,
    pub env: Option<Vec<String>>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    pub pass_through_env: Option<Vec<String>>,
    pub persistent: Option<bool>,
    // v2
    pub output_logs: Option<TurboOutputMode>,
    // v1 (removed)
    pub dot_env: Option<Vec<String>>,
    pub output_mode: Option<TurboOutputMode>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboJson {
    pub extends: Option<Vec<String>>,
    pub global_dependencies: Option<Vec<String>>,
    pub global_env: Option<Vec<String>>,
    pub global_pass_through_env: Option<Vec<String>>,
    // v2
    pub tasks: Option<FxHashMap<String, TurboTask>>,
    // v1 (removed)
    pub global_dot_env: Option<Vec<String>>,
    pub pipeline: Option<FxHashMap<String, TurboTask>>,
}

#[derive(Default, Deserialize)]
pub struct PackageJson {
    pub name: Option<String>,
}
