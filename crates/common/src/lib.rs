pub mod download;
pub mod project_graph;

pub fn map_miette_error(error: impl std::fmt::Display) -> extism_pdk::Error {
    moon_pdk::anyhow!("{error}")
}
