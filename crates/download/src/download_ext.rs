use extism_pdk::*;
use moon_extension_common::download::download_from_url;
use moon_pdk::{
    anyhow, args::*, extension::*, host_log, plugin_err, virtual_path, HostLogInput, HostLogTarget,
    VirtualPath,
};

#[host_fn]
extern "ExtismHost" {
    fn host_log(input: Json<HostLogInput>);
    fn to_virtual_path(path: String) -> String;
}

#[derive(Args)]
pub struct DownloadExtensionArgs {
    #[arg(long, short = 'u', required = true)]
    pub url: String,

    #[arg(long, short = 'd')]
    pub dest: Option<String>,

    #[arg(long)]
    pub name: Option<String>,
}

#[plugin_fn]
pub fn execute_extension(Json(input): Json<ExecuteExtensionInput>) -> FnResult<()> {
    let args = parse_args::<DownloadExtensionArgs>(&input.args)?;

    if !args.url.starts_with("http") {
        return Err(plugin_err!("A valid URL is required for downloading."));
    }

    // Determine destination directory
    debug!("Determining destination directory");

    let dest_dir = virtual_path!(
        buf,
        input
            .context
            .get_absolute_path(args.dest.as_deref().unwrap_or_default())
    );

    if dest_dir.exists() && dest_dir.is_file() {
        return Err(plugin_err!(
            "Destination <path>{}</path> must be a directory, found a file.",
            dest_dir.real_path().display(),
        ));
    }

    debug!(
        "Destination <path>{}</path> will be used",
        dest_dir.real_path().display(),
    );

    // Attempt to download the file
    host_log!(stdout, "Downloading <url>{}</url>", args.url);

    let dest_file = download_from_url(&args.url, &dest_dir, args.name.as_deref())?;

    host_log!(
        stdout,
        "Downloaded to <path>{}</path>",
        dest_file.real_path().display()
    );

    Ok(())
}
