use crate::nx_migrator::NxMigrator;
use extism_pdk::*;
use moon_pdk::*;
use starbase_utils::{fs, glob, json, yaml};

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
    fn host_log(input: Json<HostLogInput>);
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let mut migrator = NxMigrator::new(&input.context)?;

    // Migrate the workspace config first, so we can handle projects
    let workspace_config_path = migrator.workspace_root.join("workspace.json");

    if workspace_config_path.exists() {
        host_log!(
            stdout,
            "Migrating workspace config <file>workspace.json</file>",
        );

        migrator.migrate_workspace_config(json::read_file(&workspace_config_path)?)?;

        fs::remove(workspace_config_path)?;
    }

    // Then the root nx config second, to handle project defaults
    let root_config_path = migrator.workspace_root.join("nx.json");

    if root_config_path.exists() {
        host_log!(stdout, "Migrating root config <file>nx.json</file>",);

        migrator.migrate_root_config(json::read_file(&root_config_path)?)?;

        fs::remove(root_config_path)?;
    }

    // And lastly, all project configs (and package json to)
    for project_config_path in glob::walk_files(
        &migrator.workspace_root,
        [
            "**/*/package.json",
            "**/*/project.json",
            "!**/node_modules/**/*",
        ],
    )? {
        let project_source = project_config_path
            .parent()
            .unwrap()
            .strip_prefix(&migrator.workspace_root)
            .unwrap()
            .to_string_lossy();

        host_log!(
            stdout,
            "Migrating project config <file>{}</file>",
            project_config_path
                .strip_prefix(&migrator.workspace_root)
                .unwrap()
                .display()
        );

        if project_config_path
            .file_name()
            .is_some_and(|name| name == "package.json")
        {
            migrator.migrate_project_package_config(
                &project_source,
                json::read_file(&project_config_path)?,
            )?;

            // Don't delete package.json
        } else {
            migrator
                .migrate_project_config(&project_source, json::read_file(&project_config_path)?)?;

            fs::remove(project_config_path)?;
        }
    }

    // Fill in any missing but required settings
    migrator.use_default_settings()?;

    // Write the new config files
    if migrator.tasks_config_modified {
        yaml::write_file(migrator.tasks_config_path, &migrator.tasks_config)?;
    }

    if migrator.workspace_config_modified {
        yaml::write_file(migrator.workspace_config_path, &migrator.workspace_config)?;
    }

    for (project_config_path, project_config) in migrator.project_configs {
        yaml::write_file(project_config_path, &project_config)?;
    }

    host_log!(stdout, "Successfully migrated from Nx to moon!");

    Ok(())
}
