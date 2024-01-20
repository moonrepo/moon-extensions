use extism_pdk::debug;
use moon_pdk::{fetch_url_bytes, AnyResult, VirtualPath};
use std::fs;

pub fn download_from_url<U: AsRef<str>, P: AsRef<VirtualPath>>(
    url: U,
    dir: P,
) -> AnyResult<VirtualPath> {
    let url = url.as_ref();
    let dir = dir.as_ref();

    debug!("Downloading file from <url>{}</url>", url);

    // Extract the file name from the URL
    let last_sep = url.rfind('/').unwrap();
    let file_name = &url[last_sep + 1..];

    // Fetch the bytes of the URL
    let bytes = fetch_url_bytes(url)?;

    // Write the to the provided file
    let file = dir.join(file_name);

    fs::create_dir_all(dir)?;
    fs::write(&file, bytes)?;

    debug!("Downloaded to <path>{}</path>", file.display());

    Ok(file)
}
