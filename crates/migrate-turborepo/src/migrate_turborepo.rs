use crate::turbo_json::*;
use extism_pdk::*;
use moon_common::Id;
use moon_config::{
    FilePath, InputPath, OutputPath, PartialInheritedTasksConfig, PartialProjectConfig,
    PartialTaskArgs, PartialTaskConfig, PartialTaskDependency, PartialTaskOptionsConfig,
    PlatformType, PortablePath, TaskOptionEnvFile, TaskOutputStyle,
};
use moon_extension_common::map_miette_error;
use moon_pdk::{extension::*, host_log, AnyResult, HostLogInput, HostLogTarget};
use moon_target::Target;
use rustc_hash::FxHashMap;
use starbase_utils::json;
use std::collections::BTreeMap;
use std::str::FromStr;

#[host_fn]
extern "ExtismHost" {
    fn host_log(input: Json<HostLogInput>);
    fn to_virtual_path(path: String) -> String;
}

fn migrate_task(turbo_task: TurboTask) -> AnyResult<PartialTaskConfig> {
    let mut config = PartialTaskConfig::default();
    let mut inputs = vec![];

    // TODO
    config.command = Some(PartialTaskArgs::String(format!(
        "moon node run-script {name}"
    )));

    // Dependencies
    if let Some(depends_on) = turbo_task.depends_on {
        let mut deps: Vec<Target> = vec![];

        for dep in depends_on {
            let dep_target = if dep.starts_with('^') {
                dep.replace('^', "^:")
            } else if dep.contains('#') {
                dep.replace('#', ":")
            } else if dep.starts_with('$') {
                inputs.push(InputPath::from_str(&dep)?);
                continue;
            } else {
                dep
            };

            deps.push(Target::parse(&dep_target).map_err(map_miette_error)?);
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
    if let Some(env_vars) = turbo_task.env {
        for env in env_vars {
            inputs.push(InputPath::EnvVar(env));
        }
    }

    if let Some(raw_inputs) = turbo_task.inputs {
        for input in raw_inputs {
            if input == "$TURBO_DEFAULT$" {
                continue;
            }

            inputs.push(InputPath::from_str(&input)?);
        }
    }

    if !inputs.is_empty() {
        config.inputs = Some(inputs);
    }

    // Outputs
    if let Some(raw_outputs) = turbo_task.outputs {
        let mut outputs = vec![];

        for output in raw_outputs {
            if output.ends_with("/**") {
                outputs.push(OutputPath::ProjectGlob(format!("{output}/*")));
            } else {
                outputs.push(OutputPath::from_str(&output)?);
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

    if let Some(dot_env) = turbo_task.dot_env {
        config
            .options
            .get_or_insert(PartialTaskOptionsConfig::default())
            .env_file = Some(if dot_env.len() == 1 {
            TaskOptionEnvFile::File(FilePath::from_str(&dot_env[0])?)
        } else {
            TaskOptionEnvFile::Enabled(true)
        });
    }

    if let Some(output_mode) = turbo_task.output_mode {
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

fn migrate_root_globals(
    global_config: &mut PartialInheritedTasksConfig,
    turbo_json: &TurboJson,
) -> AnyResult<bool> {
    let mut modified = false;

    if let Some(global_deps) = &turbo_json.global_dependencies {
        let implicit_inputs = global_config.implicit_inputs.get_or_insert(vec![]);

        for dep in global_deps {
            implicit_inputs.push(InputPath::from_str(dep)?);
        }

        modified = true;
    }

    if let Some(global_dot_env) = &turbo_json.global_dot_env {
        let implicit_inputs = global_config.implicit_inputs.get_or_insert(vec![]);

        for env_file in global_dot_env {
            implicit_inputs.push(InputPath::from_str(env_file)?);
        }

        modified = true;
    }

    if let Some(global_env) = &turbo_json.global_env {
        let implicit_inputs = global_config.implicit_inputs.get_or_insert(vec![]);

        for env in global_env {
            implicit_inputs.push(InputPath::EnvVar(env.to_owned()));
        }

        modified = true;
    }

    Ok(modified)
}

fn migrate_project_config(
    project_configs: &mut FxHashMap<String, PartialProjectConfig>,
    turbo_task: TurboTask,
    task_id: &str,
    project_source: &str,
) -> AnyResult<()> {
    project_configs
        .entry(project_source.into())
        .or_default()
        .tasks
        .get_or_insert(BTreeMap::default())
        .insert(Id::new(task_id)?, migrate_task(turbo_task)?);

    Ok(())
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let mut global_tasks_config = PartialInheritedTasksConfig::default();
    let mut project_tasks_configs: FxHashMap<String, PartialProjectConfig> = FxHashMap::default();
    let mut has_modified_global_tasks = false;

    // Migrate the workspace root config first
    let root_config_path = input.context.workspace_root.join("turbo.json");

    if root_config_path.exists() {
        host_log!(
            stdout,
            "Migrate root config <path>{}</path>",
            root_config_path.real_path().display()
        );

        let root_config: TurboJson = json::read_file(&root_config_path)?;

        if migrate_root_globals(&mut global_tasks_config, &root_config)? {
            has_modified_global_tasks = true;
        }

        for (id, task) in root_config.pipeline {
            // Root-level task
            if let Some(task_id) = id.strip_prefix("//#") {
                migrate_project_config(&mut project_tasks_configs, task, task_id, "")?;

                continue;
            }
        }
    }

    Ok(())
}
