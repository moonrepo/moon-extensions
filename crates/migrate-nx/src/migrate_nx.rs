use crate::workspace_migrator::NxWorkspaceMigrator;
use extism_pdk::*;
use moon_pdk::*;
use starbase_utils::{fs, json};

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
    fn host_log(input: Json<HostLogInput>);
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let mut migrator = NxWorkspaceMigrator::new(&input.context)?;

    // Migrate the workspace root configs first
    let root_config_path = migrator.workspace_root.join("nx.json");

    if root_config_path.exists() {
        host_log!(
            stdout,
            "Migrating root config <file>{}</file>",
            root_config_path
                .strip_prefix(&migrator.workspace_root)
                .unwrap()
                .display()
        );

        migrator.migrate_root_config(json::read_file(&root_config_path)?)?;

        fs::remove(root_config_path)?;
    }

    let workspace_config_path = migrator.workspace_root.join("workspace.json");

    if workspace_config_path.exists() {
        host_log!(
            stdout,
            "Migrating workspace config <file>{}</file>",
            workspace_config_path
                .strip_prefix(&migrator.workspace_root)
                .unwrap()
                .display()
        );

        migrator.migrate_workspace_config(json::read_file(&workspace_config_path)?)?;

        fs::remove(workspace_config_path)?;
    }

    Ok(())
}
