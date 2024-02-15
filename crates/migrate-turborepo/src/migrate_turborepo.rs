use crate::turbo_json::*;
use extism_pdk::*;
use moon_config::{
    FilePath, InputPath, OutputPath, PartialTaskArgs, PartialTaskConfig, PartialTaskDependency,
    PartialTaskOptionsConfig, PlatformType, PortablePath, TaskOptionEnvFile, TaskOutputStyle,
};
use moon_extension_common::migrator::*;
use moon_extension_common::project_graph::*;
use moon_pdk::*;
use moon_target::Target;
use starbase_utils::{fs, json};
use std::collections::BTreeMap;
use std::str::FromStr;

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
    fn host_log(input: Json<HostLogInput>);
}

#[derive(Args)]
pub struct MigrateTurborepoExtensionArgs {
    #[arg(long)]
    pub bun: bool,
}

struct TurboMigrator {
    pub inner: Migrator,
    pub package_manager: String,
    pub project_graph: ProjectGraph,
}

impl TurboMigrator {
    pub fn new(args: &MigrateTurborepoExtensionArgs, context: &MoonContext) -> AnyResult<Self> {
        let mut migrator = Migrator::new(&context.workspace_root)?;
        let mut package_manager = "npm";

        if args.bun || context.workspace_root.join("bun.lockb").exists() {
            migrator.platform = PlatformType::Bun;
            package_manager = "bun";
        } else if context.workspace_root.join("pnpm-lock.yaml").exists() {
            package_manager = "pnpm";
        } else if context.workspace_root.join("yarn.lock").exists() {
            package_manager = "yarn";
        }

        // Load project graph information first
        let project_graph_result =
            exec_command!("moon", ["project-graph", "--json", "--log", "off"]);
        let project_graph: ProjectGraph = json::from_str(&project_graph_result.stdout)?;

        Ok(Self {
            inner: migrator,
            package_manager: package_manager.to_owned(),
            project_graph,
        })
    }

    fn find_project_task_from_script(&self, script: &str) -> AnyResult<(&Project, String)> {
        let mut parts = script.split('#');
        let package_name = parts.next().unwrap();
        let task_id = parts.next().unwrap();
        let project = self.find_project_in_graph(package_name)?;

        Ok((project, task_id.to_owned()))
    }

    fn find_project_in_graph(&self, package_name: &str) -> AnyResult<&Project> {
        for project in self.project_graph.projects.values() {
            if project.id == package_name
                || project
                    .alias
                    .as_ref()
                    .is_some_and(|alias| alias == package_name)
            {
                return Ok(project);
            }
        }

        Err(anyhow!("Unable to migrate task for package <id>{package_name}</id>. Has the project been configured in <property>projects</property> in <file>.moon/workspace.yml</file>?"))
    }

    fn migrate_root_config(&mut self, mut turbo_json: TurboJson) -> AnyResult<()> {
        let mut implicit_inputs = vec![];

        if let Some(global_deps) = turbo_json.global_dependencies.take() {
            for dep in global_deps {
                implicit_inputs.push(InputPath::from_str(&dep)?);
            }
        }

        if let Some(global_dot_env) = turbo_json.global_dot_env.take() {
            for env_file in global_dot_env {
                implicit_inputs.push(InputPath::from_str(&env_file)?);
            }
        }

        if let Some(global_env) = turbo_json.global_env.take() {
            for env in global_env {
                implicit_inputs.push(InputPath::EnvVar(env.to_owned()));
            }
        }

        if !implicit_inputs.is_empty() {
            self.inner
                .load_tasks_platform_config()?
                .implicit_inputs
                .get_or_insert(vec![])
                .extend(implicit_inputs);
        }

        self.migrate_pipeline(turbo_json, None)?;

        Ok(())
    }

    fn migrate_project_config(
        &mut self,
        project_source: &str,
        turbo_json: TurboJson,
    ) -> AnyResult<()> {
        self.migrate_pipeline(turbo_json, Some(project_source))
    }

    fn migrate_pipeline(
        &mut self,
        turbo_json: TurboJson,
        from_source: Option<&str>,
    ) -> AnyResult<()> {
        let Some(pipeline) = turbo_json.pipeline else {
            return Ok(());
        };

        // package.json script names to turbo tasks
        for (script, task) in pipeline {
            let project_source;
            let script_name;

            // Root-level task
            if let Some(root_script) = script.strip_prefix("//#") {
                project_source = String::new();
                script_name = root_script.to_owned();
            }
            // Project-scoped task
            else if script.contains('#') {
                (project_source, script_name) = self
                    .find_project_task_from_script(&script)
                    .map(|(p, i)| (p.source.to_owned(), i))?;
            }
            // For a source task
            else if let Some(source) = from_source {
                project_source = source.to_owned();
                script_name = script;
            }
            // Global task
            else {
                let task = self.migrate_task(task, &script)?;
                let task_id = create_id(&script)?;

                self.inner
                    .load_tasks_platform_config()?
                    .tasks
                    .get_or_insert(BTreeMap::default())
                    .insert(task_id, task);

                continue;
            }

            let task = self.migrate_task(task, &script_name)?;
            let task_id = create_id(&script_name)?;

            self.inner
                .load_project_config(&project_source)?
                .tasks
                .get_or_insert(BTreeMap::default())
                .insert(task_id, task);
        }

        Ok(())
    }

    fn migrate_task(
        &self,
        turbo_task: TurboTask,
        package_script: &str,
    ) -> AnyResult<PartialTaskConfig> {
        let mut config = PartialTaskConfig::default();
        let mut inputs = vec![];

        config.command = Some(PartialTaskArgs::String(format!(
            "{} run {}",
            self.package_manager, package_script
        )));

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
                    let (package, script) = self.find_project_task_from_script(dep)?;
                    let task_id = create_id(&script)?;

                    deps.push(
                        Target::parse(format!("{}:{task_id}", package.id).as_str())
                            .map_err(map_miette_error)?,
                    );
                    continue;
                }

                // ~:task
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
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let args = parse_args::<MigrateTurborepoExtensionArgs>(&input.args)?;
    let workspace_root = &input.context.workspace_root;
    let mut migrator = TurboMigrator::new(&args, &input.context)?;

    // Migrate the workspace root config first
    let root_config_path = workspace_root.join("turbo.json");

    if root_config_path.exists() {
        host_log!(stdout, "Migrating root config <file>turbo.json</file>",);

        migrator.migrate_root_config(json::read_file(&root_config_path)?)?;

        fs::remove(root_config_path)?;
    }

    // Then migrate project configs
    let project_sources = migrator
        .project_graph
        .projects
        .values()
        .map(|p| p.source.clone())
        .collect::<Vec<_>>();

    for project_source in project_sources {
        let project_config_path = workspace_root.join(&project_source).join("turbo.json");

        if project_config_path.exists() {
            host_log!(
                stdout,
                "Migrating project config <file>{}</file>",
                format!("{}/turbo.json", project_source),
            );

            migrator
                .migrate_project_config(&project_source, json::read_file(&project_config_path)?)?;

            fs::remove(project_config_path)?;
        }
    }

    // Write the new config files
    migrator.inner.save_configs()?;

    host_log!(stdout, "Successfully migrated from Turborepo to moon!");

    Ok(())
}
