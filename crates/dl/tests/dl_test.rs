use moon_pdk_test_utils::{create_extension, ExecuteExtensionInput};
use starbase_sandbox::create_empty_sandbox;
use std::fs;

mod dl {
    use super::*;

    #[test]
    #[should_panic(expected = "the following required arguments were not provided")]
    fn errors_if_no_args() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![],
            context: plugin.create_context(sandbox.path()),
        });
    }

    #[test]
    #[should_panic(expected = "A valid URL is required for downloading.")]
    fn errors_if_not_a_url() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec!["--url".into(), "invalid".into()],
            context: plugin.create_context(sandbox.path()),
        });
    }

    #[test]
    #[should_panic(expected = "must be a directory, found a file")]
    fn errors_if_dest_is_a_file() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        sandbox.create_file("dest", "file");

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![
                "--url".into(),
                "https://raw.githubusercontent.com/moonrepo/moon/master/README.md".into(),
                "--dest".into(),
                "./dest".into(),
            ],
            context: plugin.create_context(sandbox.path()),
        });
    }

    #[test]
    fn downloads_file() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![
                "--url".into(),
                "https://raw.githubusercontent.com/moonrepo/moon/master/README.md".into(),
                "--dest".into(),
                ".".into(),
            ],
            context: plugin.create_context(sandbox.path()),
        });

        let file = sandbox.path().join("README.md");

        assert!(file.exists());
        assert_eq!(fs::metadata(file).unwrap().len(), 4013);
    }

    #[test]
    fn downloads_file_to_subdir() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![
                "--url".into(),
                "https://raw.githubusercontent.com/moonrepo/moon/master/README.md".into(),
                "--dest".into(),
                "./sub/dir".into(),
            ],
            context: plugin.create_context(sandbox.path()),
        });

        assert!(sandbox.path().join("sub/dir/README.md").exists());
    }

    #[test]
    fn downloads_file_with_custom_name() {
        let sandbox = create_empty_sandbox();
        let plugin = create_extension("test", sandbox.path());

        plugin.execute_extension(ExecuteExtensionInput {
            args: vec![
                "--url".into(),
                "https://raw.githubusercontent.com/moonrepo/moon/master/README.md".into(),
                "--dest".into(),
                "./sub/dir".into(),
                "--name".into(),
                "moon.md".into(),
            ],
            context: plugin.create_context(sandbox.path()),
        });

        assert!(sandbox.path().join("sub/dir/moon.md").exists());
    }
}
