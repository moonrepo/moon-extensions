use extism_pdk::*;
use moon_extension_common::download::download_from_url;
use moon_pdk::{anyhow, args::*, extension::*, plugin_err, virtual_path, VirtualPath};
use starbase_archive::Archiver;
use std::fs;

#[host_fn]
extern "ExtismHost" {
    fn to_virtual_path(path: String) -> String;
}

#[derive(Args)]
pub struct ExecuteExtensionArgs {
    #[arg(long, short = 'i', required = true)]
    pub input: String,

    #[arg(long, short = 'o', required = true)]
    pub output: String,

    #[arg(long)]
    pub prefix: Option<String>,
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let args = parse_args::<ExecuteExtensionArgs>(&input.args)?;

    // Determine the correct input. If the input is a URL, attempt to download
    // the file, otherwise use the file directly (if within our whitelist).
    let input_file = if args.input.starts_with("http") {
        debug!("Received a URL as the input source");

        download_from_url(&args.input, virtual_path!("/moon/temp"), None)?
    } else {
        debug!(
            "Converting input <file>{}</file> to an absolute virtual path",
            args.input
        );

        virtual_path!(buf, input.context.get_absolute_path(args.input))
    };

    if !input_file
        .extension()
        .is_some_and(|ext| ext == "tar" || ext == "tgz" || ext == "gz" || ext == "zip")
    {
        return Err(plugin_err!(
            "Invalid input, only <file>tar (gz)</file> and <file>zip</file> archives are supported."
        ));
    }

    if !input_file.exists() || !input_file.is_file() {
        return Err(plugin_err!(
            "Input <path>{}</path> must be a valid file.",
            input_file.display(),
        ));
    }

    info!(
        "Opening archive <path>{}</path>",
        input_file.real_path().display()
    );

    // Convert the provided output into a virtual file path.
    let output_dir = virtual_path!(buf, input.context.get_absolute_path(args.output));

    if output_dir.exists() && output_dir.is_file() {
        return Err(plugin_err!(
            "Output <path>{}</path> must be a directory, found a file.",
            output_dir.real_path().display(),
        ));
    }

    if input_file == output_dir {
        return Err(plugin_err!(
            "Input and output cannot point to the same location."
        ));
    }

    fs::create_dir_all(&output_dir)?;

    info!(
        "Unpacking archive to <path>{}</path>",
        output_dir.real_path().display()
    );

    // Attempt to unpack the archive!
    let mut archive = Archiver::new(&output_dir, &input_file);

    // Diff against all files in the output dir
    archive.add_source_glob("**/*");

    // Remove the prefix from unpacked files
    if let Some(prefix) = &args.prefix {
        archive.set_prefix(prefix);
    }

    // Unpack the files
    archive
        .unpack_from_ext()
        .map_err(|error| anyhow!("{error}"))?;

    info!("Unpacked archive!");

    Ok(())
}
