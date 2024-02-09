use moon_pdk_test_utils::{create_extension, ExecuteExtensionInput};
use starbase_sandbox::{assert_snapshot, create_empty_sandbox, create_sandbox};
use std::fs;

mod migrate_nx {
    use super::*;

    #[test]
    fn converts_root_files() {
        let sandbox = create_sandbox("root");
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![],
            context: plugin.create_context(sandbox.path()),
        });

        assert!(!sandbox.path().join("nx.json").exists());
        assert!(!sandbox.path().join("workspace.json").exists());
        assert!(sandbox.path().join(".moon/tasks/node.yml").exists());
        assert!(sandbox.path().join(".moon/workspace.yml").exists());

        assert_snapshot!(fs::read_to_string(sandbox.path().join(".moon/tasks/node.yml")).unwrap());
        assert_snapshot!(fs::read_to_string(sandbox.path().join(".moon/workspace.yml")).unwrap());
    }

    mod projects {
        use super::*;

        #[test]
        fn uses_defaults() {
            let sandbox = create_empty_sandbox();
            let plugin = create_extension("test", sandbox.path());

            plugin.execute_extension(ExecuteExtensionInput {
                args: vec![],
                context: plugin.create_context(sandbox.path()),
            });

            assert_snapshot!(
                fs::read_to_string(sandbox.path().join(".moon/workspace.yml")).unwrap()
            );
        }

        #[test]
        fn inherits_layout() {
            let sandbox = create_empty_sandbox();
            sandbox.create_file(
                "nx.json",
                r#"
{
  "workspaceLayout": {
    "appsDir": "applications",
    "libsDir": "libraries"
  }
}"#,
            );

            let plugin = create_extension("test", sandbox.path());

            plugin.execute_extension(ExecuteExtensionInput {
                args: vec![],
                context: plugin.create_context(sandbox.path()),
            });

            assert_snapshot!(
                fs::read_to_string(sandbox.path().join(".moon/workspace.yml")).unwrap()
            );
        }
    }
}
