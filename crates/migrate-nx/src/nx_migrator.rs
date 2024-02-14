use crate::nx_json::*;
use crate::nx_project_json::{NxProjectJson, PackageJsonWithNx};
use moon_common::Id;
use moon_config::{
    InputPath, LanguageType, OutputPath, PartialInheritedTasksConfig, PartialProjectConfig,
    PartialProjectDependsOn, PartialTaskArgs, PartialTaskConfig, PartialTaskDependency,
    PartialTaskOptionsConfig, PartialVcsConfig, PartialWorkspaceConfig, PartialWorkspaceProjects,
    PlatformType, ProjectType,
};
use moon_pdk::{map_miette_error, AnyResult, MoonContext, VirtualPath};
use moon_target::Target;
use rustc_hash::FxHashMap;
use starbase_utils::{json::JsonValue, yaml};
use std::collections::BTreeMap;
use std::str::FromStr;

pub struct NxMigrator {
    pub context: MoonContext,
    pub project_configs: FxHashMap<VirtualPath, PartialProjectConfig>,
    pub tasks_config: PartialInheritedTasksConfig,
    pub tasks_config_path: VirtualPath,
    pub tasks_config_modified: bool,
    pub workspace_config: PartialWorkspaceConfig,
    pub workspace_config_path: VirtualPath,
    pub workspace_config_modified: bool,
    pub workspace_root: VirtualPath,
}

impl NxMigrator {
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
            project_configs: FxHashMap::default(),
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
                        file_groups.insert(Id::clean(name)?, group);
                    }
                }
            }
        }

        if let Some(target_defaults) = nx_json.target_defaults {
            let tasks = self.tasks_config.tasks.get_or_insert(BTreeMap::default());

            for (name, target_config) in target_defaults {
                tasks.insert(Id::clean(name)?, migrate_task(&target_config)?);
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
            projects.insert(Id::clean(key)?, value);
        }

        if !projects.is_empty() {
            self.workspace_config_modified = true;
            self.workspace_config.projects = Some(PartialWorkspaceProjects::Sources(projects));
        }

        Ok(())
    }

    pub fn migrate_project_config(
        &mut self,
        project_source: &str,
        nx_project: NxProjectJson,
    ) -> AnyResult<()> {
        let config = self.load_project_config(project_source)?;

        if let Some(implicit_dependencies) = nx_project.implicit_dependencies {
            if !implicit_dependencies.is_empty() {
                let depends_on = config.depends_on.get_or_insert(vec![]);

                for dep in implicit_dependencies {
                    depends_on.push(PartialProjectDependsOn::String(Id::clean(dep)?));
                }
            }
        }

        if let Some(named_inputs) = nx_project.named_inputs {
            if !named_inputs.is_empty() {
                let file_groups = config.file_groups.get_or_insert(FxHashMap::default());

                for (name, raw_inputs) in named_inputs {
                    let group = migrate_inputs(&raw_inputs)?;

                    if !group.is_empty() {
                        file_groups.insert(Id::clean(name)?, group);
                    }
                }
            }
        }

        if let Some(project_type) = nx_project.project_type {
            if project_type == "library" {
                config.type_of = Some(ProjectType::Library);
            } else if project_type == "application" {
                config.type_of = Some(ProjectType::Application);
            }
        }

        if let Some(raw_tags) = nx_project.tags {
            let tags = config.tags.get_or_insert(vec![]);

            for tag in raw_tags {
                tags.push(Id::clean(tag)?);
            }
        }

        Ok(())
    }

    pub fn migrate_project_package_config(
        &mut self,
        project_source: &str,
        nx_package: PackageJsonWithNx,
    ) -> AnyResult<()> {
        if let Some(nx) = nx_package.nx {
            self.migrate_project_config(project_source, nx)?;
        }

        Ok(())
    }

    fn load_project_config(
        &mut self,
        project_source: &str,
    ) -> AnyResult<&mut PartialProjectConfig> {
        let project_config_path = self.workspace_root.join(project_source).join("moon.yml");

        if !self.project_configs.contains_key(&project_config_path) {
            if project_config_path.exists() {
                self.project_configs.insert(
                    project_config_path.clone(),
                    yaml::read_file(&project_config_path)?,
                );
            } else {
                self.project_configs.insert(
                    project_config_path.clone(),
                    PartialProjectConfig {
                        language: Some(
                            if self
                                .workspace_root
                                .join(project_source)
                                .join("tsconfig.json")
                                .exists()
                            {
                                LanguageType::TypeScript
                            } else {
                                LanguageType::JavaScript
                            },
                        ),
                        platform: Some(PlatformType::Node),
                        ..PartialProjectConfig::default()
                    },
                );
            }
        }

        Ok(self.project_configs.get_mut(&project_config_path).unwrap())
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

fn migrate_task(nx_target: &NxTargetOptions) -> AnyResult<PartialTaskConfig> {
    let mut config = PartialTaskConfig::default();
    let mut inputs = vec![];

    config.command = Some(PartialTaskArgs::String(
        nx_target
            .command
            .clone()
            .or_else(|| {
                nx_target
                    .executor
                    .as_ref()
                    .map(|e| format!("nx-compat execute {e}"))
            })
            .unwrap_or("noop".into()),
    ));

    // Arguments
    // This is a bit complicated, since moon doesn't have a concept of arbitrary
    // options, but in Nx, options can also be passed as command line args,
    // so let's replicate that and hope it works correctly!
    if let Some(options) = &nx_target.options {
        let mut args = vec![];

        for (key, value) in options {
            if matches!(value, JsonValue::Null) {
                continue;
            }

            args.push(format!("--{key}"));
            args.push(value.to_string());
        }

        if !args.is_empty() {
            config.args = Some(PartialTaskArgs::List(args));
        }
    }

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

    if nx_target.command.is_some() {
        config.platform = Some(PlatformType::System);
    }

    if nx_target.cache == Some(true) {
        config
            .options
            .get_or_insert(PartialTaskOptionsConfig::default())
            .cache = nx_target.cache;
    }

    Ok(config)
}
