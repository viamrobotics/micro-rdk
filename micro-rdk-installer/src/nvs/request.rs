use super::super::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/*
This module contains the logic for acquiring the security credentials for a robot from
the Viam App and preparing for flash storage
*/

const RELEASES_BASE_URL: &str = "https://github.com/viamrobotics/micro-rdk/releases";
const BINARY_NAME: &str = "micro-rdk-server-esp32.bin";

pub async fn download_micro_rdk_release(
    path: &Path,
    version: Option<String>,
    url: Option<reqwest::Url>,
) -> Result<PathBuf, Error> {
    let release_url = if let Some(url) = url {
        url.to_string()
    } else if version.is_some() && version.clone().unwrap() != "latest" {
        format!(
            "{}/download/{}/{}",
            RELEASES_BASE_URL,
            version.unwrap(),
            BINARY_NAME
        )
    } else {
        format!(
            "{}/{}/{}",
            RELEASES_BASE_URL, "latest/download", BINARY_NAME
        )
    };

    log::info!("Downloading micro-RDK release from {:?}", release_url);
    let fname = path.to_path_buf();
    let mut dest = File::create(fname.clone()).map_err(Error::FileError)?;
    let response = reqwest::get(release_url)
        .await
        .map_err(Error::BinaryRetrievalError)?;
    response
        .error_for_status_ref()
        .map_err(Error::BinaryRetrievalError)?;
    let content = response
        .bytes()
        .await
        .map_err(Error::BinaryRetrievalError)?;
    dest.write_all(&content).map_err(Error::FileError)?;
    Ok(fname)
}
