use moon_pdk_test_utils::{create_extension, ExecuteExtensionInput};
use starbase_sandbox::{assert_snapshot, create_sandbox};
use std::fs;

mod migrate_turborepo {
    use super::*;

    #[test]
    fn converts_basic_root_file() {
        let sandbox = create_sandbox("root-only");
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![],
            context: plugin.create_context(sandbox.path()),
        });

        assert!(!sandbox.path().join("turbo.json").exists());
        assert!(sandbox.path().join(".moon/tasks/node.yml").exists());

        assert_snapshot!(fs::read_to_string(sandbox.path().join(".moon/tasks/node.yml")).unwrap());
    }
}