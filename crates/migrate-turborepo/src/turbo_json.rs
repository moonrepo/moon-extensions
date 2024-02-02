use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum TurboOutputMode {
    #[default]
    Full,
    HashOnly,
    NewOnly,
    ErrorsOnly,
    None,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboTask {
    pub cache: Option<bool>,
    pub depends_on: Option<Vec<String>>,
    pub dot_env: Option<Vec<String>>,
    pub env: Option<Vec<String>>,
    pub inputs: Option<Vec<String>>,
    pub output_mode: Option<TurboOutputMode>,
    pub outputs: Option<Vec<String>>,
    pub pass_through_env: Option<Vec<String>>,
    pub persistent: Option<bool>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurboJson {
    pub extends: Option<Vec<String>>,
    pub global_dependencies: Option<Vec<String>>,
    pub global_dot_env: Option<Vec<String>>,
    pub global_env: Option<Vec<String>>,
    pub global_pass_through_env: Option<Vec<String>>,
    pub pipeline: FxHashMap<String, TurboTask>,
}
