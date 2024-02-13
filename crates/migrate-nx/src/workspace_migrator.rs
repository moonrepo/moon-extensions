use crate::nx_json::*;
use moon_common::Id;
use moon_config::{
    InputPath, OutputPath, PartialInheritedTasksConfig, PartialTaskArgs, PartialTaskConfig,
    PartialTaskDependency, PartialTaskOptionsConfig, PartialVcsConfig, PartialWorkspaceConfig,
    PartialWorkspaceProjects,
};
use moon_pdk::{map_miette_error, AnyResult, MoonContext, VirtualPath};
use moon_target::Target;
use rustc_hash::FxHashMap;
use starbase_utils::yaml;
use std::collections::BTreeMap;
use std::str::FromStr;

pub struct NxWorkspaceMigrator {
    pub context: MoonContext,
    pub tasks_config: PartialInheritedTasksConfig,
    pub tasks_config_path: VirtualPath,
    pub tasks_config_modified: bool,
    pub workspace_config: PartialWorkspaceConfig,
    pub workspace_config_path: VirtualPath,
    pub workspace_config_modified: bool,
    pub workspace_root: VirtualPath,
}

impl NxWorkspaceMigrator {
    pub fn new(context: &MoonContext) -> AnyResult<Self> {
        // Load global configs if it exists
        let tasks_config_path = context.workspace_root.join(".moon/tasks/node.yml");
        let tasks_config = if tasks_config_path.exists() {
            yaml::read_file(&tasks_config_path)?
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
            tasks_config,
            tasks_config_path,
            tasks_config_modified: false,
            workspace_config,
            workspace_config_path,
            workspace_config_modified: false,
            workspace_root: context.workspace_root.clone(),
        })
    }

    pub fn use_default_settings(&mut self) -> AnyResult<()> {
        if self.tasks_config.file_groups.is_none() {
            self.tasks_config_modified = true;

            let groups = self
                .tasks_config
                .file_groups
                .get_or_insert(FxHashMap::default());

            if !groups.contains_key("default") {
                groups.insert(
                    "default".into(),
                    vec![InputPath::ProjectGlob("**/*".into())],
                );
            }

            if !groups.contains_key("production") {
                groups.insert("production".into(), vec![]);
            }

            if !groups.contains_key("sharedGlobals") {
                groups.insert("sharedGlobals".into(), vec![]);
            }
        }

        if self.workspace_config.projects.is_none() {
            self.workspace_config_modified = true;
            self.workspace_config.projects = Some(PartialWorkspaceProjects::Globs(vec![
                "apps/*".into(),
                "packages/*".into(),
            ]));
        }

        Ok(())
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
                    .tasks_config
                    .file_groups
                    .get_or_insert(FxHashMap::default());

                for (name, raw_inputs) in named_inputs {
                    let group = migrate_inputs(&raw_inputs)?;

                    if !group.is_empty() {
                        self.tasks_config_modified = true;
                        file_groups.insert(Id::new(name)?, group);
                    }
                }
            }
        }

        if let Some(target_defaults) = nx_json.target_defaults {
            let tasks = self.tasks_config.tasks.get_or_insert(BTreeMap::default());

            for (name, target_config) in target_defaults {
                tasks.insert(Id::new(name)?, migrate_task(target_config)?);
            }
        }

        if self.workspace_config.projects.is_none() {
            if let Some(layout) = nx_json.workspace_layout {
                self.workspace_config_modified = true;
                self.workspace_config.projects = Some(PartialWorkspaceProjects::Globs(vec![
                    format!("{}/*", layout.apps_dir.unwrap_or("apps".into())),
                    format!("{}/*", layout.libs_dir.unwrap_or("libs".into())),
                ]));
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

fn is_path_or_glob(value: &str) -> bool {
    value.contains('/')
        || value.contains('*')
        || value.contains('.')
        || value.contains('{')
        || value.starts_with('!')
}

fn replace_tokens(value: &str) -> String {
    let mut result = value.replace("{projectName}", "$project");

    if result.starts_with("!{projectRoot}/") {
        result = result.replacen("!{projectRoot}/", "!", 1);
    }

    if result.starts_with("{projectRoot}/") {
        result = result.replacen("{projectRoot}/", "", 1);
    }

    result = result.replace("{projectRoot}", "$projectRoot");

    if result.starts_with("!{workspaceRoot}/") {
        result = result.replacen("{workspaceRoot}/", "!/", 1);
    }

    if result.starts_with("{workspaceRoot}/") {
        result = result.replacen("{workspaceRoot}/", "/", 1);
    }

    result = result.replace("{workspaceRoot}", "$workspaceRoot");

    result
}

fn migrate_inputs(raw_inputs: &[NxInput]) -> AnyResult<Vec<InputPath>> {
    let mut inputs = vec![];

    for input in raw_inputs {
        match input {
            NxInput::Dep { .. } => {
                // Not supported
            }
            NxInput::DepOutput { .. } => {
                // Not supported
            }
            NxInput::External { .. } => {
                // Not supported, moon parses lockfiles automatically
            }
            NxInput::Env { env } => {
                inputs.push(InputPath::EnvVar(env.to_owned()));
            }
            NxInput::Fileset { fileset } => {
                inputs.push(InputPath::from_str(&replace_tokens(&fileset))?);
            }
            NxInput::Runtime { .. } => {
                // Not supported, moon includes tool version automatically
            }
            NxInput::Source(source) => {
                // File path or glob
                if is_path_or_glob(source) {
                    inputs.push(InputPath::from_str(&replace_tokens(&source))?);
                }
                // Named input
                else {
                    if source.starts_with('^') {
                        // Not supported, cannot depend on sources from other projects
                    } else {
                        inputs.push(InputPath::TokenFunc(format!("@group({source})")));
                    }
                }
            }
        };
    }

    Ok(inputs)
}

fn migrate_task(nx_target: NxTargetOptions) -> AnyResult<PartialTaskConfig> {
    let mut config = PartialTaskConfig::default();
    let mut inputs = vec![];

    config.command = Some(PartialTaskArgs::String(
        nx_target
            .command
            .or_else(|| nx_target.executor.map(|e| format!("nx-compat execute {e}")))
            .unwrap_or("noop".into()),
    ));

    // Dependencies
    // - https://nx.dev/reference/project-configuration#dependson
    if let Some(depends_on) = &nx_target.depends_on {
        let mut deps: Vec<Target> = vec![];

        for dep in depends_on {
            match dep {
                NxDependsOn::Object {
                    dependencies,
                    target,
                    projects,
                    ..
                } => {
                    if let Some(projects) = projects {
                        match projects {
                            StringOrList::List(ids) => {
                                for id in ids {
                                    deps.push(Target::new(id, target).map_err(map_miette_error)?);
                                }
                            }
                            StringOrList::String(scope) => {
                                deps.push(
                                    Target::parse(
                                        format!(
                                            "{}:{target}",
                                            if scope == "self" { "~" } else { "^" }
                                        )
                                        .as_str(),
                                    )
                                    .map_err(map_miette_error)?,
                                );
                            }
                        };
                    } else if let Some(dependencies) = dependencies {
                        if *dependencies {
                            deps.push(
                                Target::parse(format!("^:{target}").as_str())
                                    .map_err(map_miette_error)?,
                            );
                        }
                    } else {
                        deps.push(Target::new_self(&target).map_err(map_miette_error)?);
                    }
                }
                NxDependsOn::String(target) => {
                    if let Some(target) = target.strip_prefix('^') {
                        deps.push(
                            Target::parse(format!("^:{target}").as_str())
                                .map_err(map_miette_error)?,
                        );
                    } else {
                        deps.push(Target::new_self(&target).map_err(map_miette_error)?);
                    }
                }
            };
        }

        if !deps.is_empty() {
            config.deps = Some(
                deps.into_iter()
                    .map(PartialTaskDependency::Target)
                    .collect(),
            );
        }
    }

    // Inputs
    // - https://nx.dev/recipes/running-tasks/configure-inputs
    // - https://nx.dev/reference/inputs
    if let Some(raw_inputs) = &nx_target.inputs {
        inputs.extend(migrate_inputs(raw_inputs)?);
    }

    if !inputs.is_empty() {
        config.inputs = Some(inputs);
    }

    // Outputs
    // - https://nx.dev/recipes/running-tasks/configure-outputs
    if let Some(raw_outputs) = &nx_target.outputs {
        let mut outputs = vec![];

        for output in raw_outputs {
            outputs.push(OutputPath::from_str(&replace_tokens(output))?);
        }

        if !outputs.is_empty() {
            config.outputs = Some(outputs);
        }
    }

    if nx_target.cache == Some(true) {
        config
            .options
            .get_or_insert(PartialTaskOptionsConfig::default())
            .cache = nx_target.cache;
    }

    Ok(config)
}
