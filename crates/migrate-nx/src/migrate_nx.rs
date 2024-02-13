use crate::workspace_migrator::NxWorkspaceMigrator;
use extism_pdk::*;
use moon_pdk::*;
use starbase_utils::{fs, json, yaml};

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
    fn host_log(input: Json<HostLogInput>);
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let mut migrator = NxWorkspaceMigrator::new(&input.context)?;

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

    // Fill in any missing but required settings
    migrator.use_default_settings()?;

    // Write the new config files
    if migrator.tasks_config_modified {
        yaml::write_file(migrator.tasks_config_path, &migrator.tasks_config)?;
    }

    if migrator.workspace_config_modified {
        yaml::write_file(migrator.workspace_config_path, &migrator.workspace_config)?;
    }

    host_log!(stdout, "Successfully migrated from Nx to moon!");

    Ok(())
}
