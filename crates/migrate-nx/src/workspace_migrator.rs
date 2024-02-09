use crate::nx_json::{NxInput, NxJson, WorkspaceJson};
use moon_common::Id;
use moon_config::{
    InputPath, PartialInheritedTasksConfig, PartialVcsConfig, PartialWorkspaceConfig,
    PartialWorkspaceProjects,
};
use moon_pdk::{AnyResult, MoonContext, VirtualPath};
use rustc_hash::FxHashMap;
use starbase_utils::yaml;
use std::str::FromStr;

pub struct NxWorkspaceMigrator {
    pub context: MoonContext,
    pub global_config: PartialInheritedTasksConfig,
    pub global_config_path: VirtualPath,
    pub global_config_modified: bool,
    pub workspace_config: PartialWorkspaceConfig,
    pub workspace_config_path: VirtualPath,
    pub workspace_config_modified: bool,
    pub workspace_root: VirtualPath,
}

impl NxWorkspaceMigrator {
    pub fn new(context: &MoonContext) -> AnyResult<Self> {
        // Load global configs if it exists
        let global_config_path = context.workspace_root.join(".moon/tasks/node.yml");
        let global_config = if global_config_path.exists() {
            yaml::read_file(&global_config_path)?
        } else {
            PartialInheritedTasksConfig::default()
        };

        let workspace_config_path = context.workspace_root.join(".moon/workspace.yml");
        let workspace_config = if workspace_config_path.exists() {
            yaml::read_file(&workspace_config_path)?
        } else {
            PartialWorkspaceConfig::default()
        };

        Ok(Self {
            context: context.to_owned(),
            global_config,
            global_config_path,
            global_config_modified: false,
            workspace_config,
            workspace_config_path,
            workspace_config_modified: false,
            workspace_root: context.workspace_root.clone(),
        })
    }

    fn convert_input(&self, input: NxInput) -> AnyResult<Option<InputPath>> {
        match input {
            NxInput::Dep {
                dependencies,
                projects,
                input,
            } => Ok(None),
            NxInput::DepOutput { .. } => Ok(None),
            NxInput::External { .. } => Ok(None),
            NxInput::Env { env } => Ok(Some(InputPath::EnvVar(env))),
            NxInput::Fileset { fileset } => Ok(Some(InputPath::from_str(&fileset)?)),
            NxInput::Runtime { .. } => Ok(None),
            NxInput::Source(path) => Ok(Some(InputPath::from_str(&path)?)),
        }
    }

    pub fn migrate_root_config(&mut self, nx_json: NxJson) -> AnyResult<()> {
        if let Some(affected) = nx_json.affected {
            if let Some(default_branch) = affected.default_base {
                self.workspace_config_modified = true;
                self.workspace_config
                    .vcs
                    .get_or_insert(PartialVcsConfig::default())
                    .default_branch = Some(default_branch);
            }
        }

        if let Some(named_inputs) = nx_json.named_inputs {
            if !named_inputs.is_empty() {
                let file_groups = self
                    .global_config
                    .file_groups
                    .get_or_insert(FxHashMap::default());

                for (name, values) in named_inputs {
                    let mut group = vec![];

                    for value in values {
                        match value {
                            NxInput::Env { env } => {
                                group.push(InputPath::EnvVar(env));
                            }
                            NxInput::Fileset { fileset } => {
                                group.push(InputPath::from_str(&replace_tokens(&fileset))?);
                            }
                            NxInput::Source(path) => {
                                if path.contains("{projectRoot}")
                                    || path.contains("{workspaceRoot}")
                                    || path.contains('/')
                                {
                                    group.push(InputPath::from_str(&replace_tokens(&path))?);
                                } else {
                                    // Ignore name references
                                }
                            }
                            _ => {
                                // Other input types are not supported by moon
                                // at the top-level or at all...
                                continue;
                            }
                        };
                    }

                    if !group.is_empty() {
                        self.global_config_modified = true;
                        file_groups.insert(Id::raw(name), group);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn migrate_workspace_config(&mut self, workspace_json: WorkspaceJson) -> AnyResult<()> {
        let mut projects = FxHashMap::default();

        for (key, value) in workspace_json.projects {
            projects.insert(Id::new(key)?, value);
        }

        if !projects.is_empty() {
            self.workspace_config_modified = true;
            self.workspace_config.projects = Some(PartialWorkspaceProjects::Sources(projects));
        }

        Ok(())
    }
}

fn replace_tokens(value: &str) -> String {
    value
        .replace("{projectRoot}/", "")
        .replace("{projectRoot}", "")
        .replace("{workspaceRoot}/", "/")
        .replace("{workspaceRoot}", "/")
}
