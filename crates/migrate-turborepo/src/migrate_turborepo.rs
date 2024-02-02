use crate::project_graph::{Project, ProjectGraph};
use crate::turbo_json::*;
use extism_pdk::*;
use moon_common::Id;
use moon_config::{
    FilePath, InputPath, OutputPath, PartialInheritedTasksConfig, PartialProjectConfig,
    PartialTaskArgs, PartialTaskConfig, PartialTaskDependency, PartialTaskOptionsConfig,
    PlatformType, PortablePath, TaskOptionEnvFile, TaskOutputStyle,
};
use moon_extension_common::map_miette_error;
use moon_pdk::{extension::*, *};
use moon_target::Target;
use rustc_hash::FxHashMap;
use starbase_utils::{fs, glob, json, yaml};
use std::collections::BTreeMap;
use std::str::FromStr;

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
    fn host_log(input: Json<HostLogInput>);
    fn to_virtual_path(path: String) -> String;
}

struct TurboMigrator {
    pub global_config: PartialInheritedTasksConfig,
    pub global_config_path: VirtualPath,
    pub global_config_modified: bool,
    pub project_configs: FxHashMap<VirtualPath, PartialProjectConfig>,
    pub project_graph: ProjectGraph,
    pub workspace_root: VirtualPath,
}

impl TurboMigrator {
    pub fn new(context: &MoonContext) -> AnyResult<Self> {
        // Load global config if it exists
        let global_config_path = context.workspace_root.join(".moon/tasks/node.yml");
        let global_config = if global_config_path.exists() {
            yaml::read_file(&global_config_path)?
        } else {
            PartialInheritedTasksConfig::default()
        };

        // Load project graph information first
        let project_graph_result = exec_command!("moon", ["project-graph", "--json"]);
        let project_graph: ProjectGraph = json::from_str(&project_graph_result.stdout)?;

        Ok(Self {
            global_config,
            global_config_path,
            global_config_modified: false,
            project_configs: FxHashMap::default(),
            project_graph,
            workspace_root: context.workspace_root.clone(),
        })
    }

    fn find_project_task_from_id(&self, id: &str) -> AnyResult<(&Project, String)> {
        let mut parts = id.split('#');
        let package_name = parts.next().unwrap();
        let task_id = parts.next().unwrap();
        let project = self.find_project_in_graph(package_name)?;

        Ok((project, task_id.to_owned()))
    }

    fn find_project_in_graph(&self, package_name: &str) -> AnyResult<&Project> {
        for project in self.project_graph.projects.values() {
            if project
                .alias
                .as_ref()
                .is_some_and(|alias| alias == package_name)
            {
                return Ok(project);
            }
        }

        Err(anyhow!("Unable to migrate task for package <id>{package_name}</id>. Has the project been configured in moon's workspace projects?"))
    }

    fn migrate_root_config(&mut self, mut turbo_json: TurboJson) -> AnyResult<()> {
        if let Some(global_deps) = turbo_json.global_dependencies.take() {
            let implicit_inputs = self.global_config.implicit_inputs.get_or_insert(vec![]);

            for dep in global_deps {
                implicit_inputs.push(InputPath::from_str(&dep)?);
            }

            self.global_config_modified = true;
        }

        if let Some(global_dot_env) = turbo_json.global_dot_env.take() {
            let implicit_inputs = self.global_config.implicit_inputs.get_or_insert(vec![]);

            for env_file in global_dot_env {
                implicit_inputs.push(InputPath::from_str(&env_file)?);
            }

            self.global_config_modified = true;
        }

        if let Some(global_env) = turbo_json.global_env.take() {
            let implicit_inputs = self.global_config.implicit_inputs.get_or_insert(vec![]);

            for env in global_env {
                implicit_inputs.push(InputPath::EnvVar(env.to_owned()));
            }

            self.global_config_modified = true;
        }

        self.migrate_pipeline(turbo_json)?;

        Ok(())
    }

    fn migrate_project_config(&mut self, turbo_json: TurboJson) -> AnyResult<()> {
        self.migrate_pipeline(turbo_json)
    }

    fn migrate_pipeline(&mut self, turbo_json: TurboJson) -> AnyResult<()> {
        for (id, task) in turbo_json.pipeline {
            let task = self.migrate_task(task, &id)?;

            // Root-level task
            if let Some(task_id) = id.strip_prefix("//#") {
                self.load_project_config("")?
                    .tasks
                    .get_or_insert(BTreeMap::default())
                    .insert(Id::new(task_id)?, task);
            }
            // Project task
            else if id.contains('#') {
                let (package_source, task_id) = self
                    .find_project_task_from_id(&id)
                    .map(|(p, i)| (p.source.to_owned(), i))?;

                self.load_project_config(&package_source)?
                    .tasks
                    .get_or_insert(BTreeMap::default())
                    .insert(Id::new(task_id)?, task);
            }
            // Global task
            else {
                self.global_config_modified = true;
                self.global_config
                    .tasks
                    .get_or_insert(BTreeMap::default())
                    .insert(Id::new(id)?, task);
            }
        }

        Ok(())
    }

    fn migrate_task(&self, turbo_task: TurboTask, task_id: &str) -> AnyResult<PartialTaskConfig> {
        let mut config = PartialTaskConfig::default();
        let mut inputs = vec![];

        // TODO package script
        config.command = Some(PartialTaskArgs::String(task_id.into()));

        // Dependencies
        if let Some(depends_on) = &turbo_task.depends_on {
            let mut deps: Vec<Target> = vec![];

            for dep in depends_on {
                // $ENV input
                if let Some(env) = dep.strip_prefix('$') {
                    inputs.push(InputPath::EnvVar(env.into()));
                    continue;
                }

                // ^:task
                if let Some(dep) = dep.strip_prefix('^') {
                    deps.push(
                        Target::parse(format!("^:{dep}").as_str()).map_err(map_miette_error)?,
                    );
                    continue;
                }

                // project:task
                if dep.contains('#') {
                    let (package, task_id) = self.find_project_task_from_id(dep)?;

                    deps.push(
                        Target::parse(format!("{}:{task_id}", package.id).as_str())
                            .map_err(map_miette_error)?,
                    );
                    continue;
                }

                // task
                deps.push(Target::parse(dep).map_err(map_miette_error)?);
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
        if let Some(env_vars) = &turbo_task.env {
            for env in env_vars {
                inputs.push(InputPath::EnvVar(env.into()));
            }
        }

        if let Some(raw_inputs) = &turbo_task.inputs {
            for input in raw_inputs {
                if input == "$TURBO_DEFAULT$" {
                    continue;
                }

                inputs.push(InputPath::from_str(input)?);
            }
        }

        if !inputs.is_empty() {
            config.inputs = Some(inputs);
        }

        // Outputs
        if let Some(raw_outputs) = &turbo_task.outputs {
            let mut outputs = vec![];

            for output in raw_outputs {
                if output.ends_with("/**") {
                    outputs.push(OutputPath::ProjectGlob(format!("{output}/*")));
                } else {
                    outputs.push(OutputPath::from_str(output)?);
                }
            }

            if !outputs.is_empty() {
                config.outputs = Some(outputs);
            }
        }

        // Options
        config.platform = Some(PlatformType::Node);

        if turbo_task.cache == Some(false) {
            config
                .options
                .get_or_insert(PartialTaskOptionsConfig::default())
                .cache = turbo_task.cache;
        }

        if let Some(dot_env) = &turbo_task.dot_env {
            config
                .options
                .get_or_insert(PartialTaskOptionsConfig::default())
                .env_file = Some(if dot_env.len() == 1 {
                TaskOptionEnvFile::File(FilePath::from_str(&dot_env[0])?)
            } else {
                TaskOptionEnvFile::Enabled(true)
            });
        }

        if let Some(output_mode) = &turbo_task.output_mode {
            let output_style = match output_mode {
                TurboOutputMode::HashOnly => Some(TaskOutputStyle::Hash),
                TurboOutputMode::NewOnly => Some(TaskOutputStyle::Buffer),
                TurboOutputMode::ErrorsOnly => Some(TaskOutputStyle::BufferOnlyFailure),
                _ => None,
            };

            if output_style.is_some() {
                config
                    .options
                    .get_or_insert(PartialTaskOptionsConfig::default())
                    .output_style = output_style;
            }
        }

        if turbo_task.persistent == Some(true) {
            config.local = turbo_task.persistent;
        }

        Ok(config)
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
                self.project_configs
                    .insert(project_config_path.clone(), PartialProjectConfig::default());
            }
        }

        Ok(self.project_configs.get_mut(&project_config_path).unwrap())
    }
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let mut migrator = TurboMigrator::new(&input.context)?;

    // Migrate the workspace root config first
    let root_config_path = migrator.workspace_root.join("turbo.json");

    if root_config_path.exists() {
        host_log!(
            stdout,
            "Migrating root config <path>{}</path>",
            root_config_path
                .strip_prefix(&migrator.workspace_root)
                .unwrap()
                .display()
        );

        migrator.migrate_root_config(json::read_file(&root_config_path)?)?;

        fs::remove(root_config_path)?;
    }

    // Then migrate project configs
    for project_config_path in glob::walk_files(&migrator.workspace_root, ["**/turbo.json"])? {
        host_log!(
            stdout,
            "Migrating project config <path>{}</path>",
            project_config_path
                .strip_prefix(&migrator.workspace_root)
                .unwrap()
                .display()
        );

        migrator.migrate_project_config(json::read_file(&project_config_path)?)?;

        fs::remove(project_config_path)?;
    }

    // Write the new config files
    if migrator.global_config_modified {
        yaml::write_file(migrator.global_config_path, &migrator.global_config)?;
    }

    for (project_config_path, project_config) in migrator.project_configs {
        yaml::write_file(project_config_path, &project_config)?;
    }

    host_log!(stdout, "Successfully migrated from Turborepo to moon!");

    Ok(())
}
