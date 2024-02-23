#[cfg(feature = "wasm")]
mod migrate_nx;
mod nx_json;
mod nx_migrator;
mod nx_project_json;

#[cfg(feature = "wasm")]
pub use migrate_nx::*;
