use moon_pdk_test_utils::{create_extension, ExecuteExtensionInput};
use starbase_sandbox::create_empty_sandbox;
use std::fs;

mod unpack {
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
}
