use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::service::SERVICE_NAME;

pub fn init() -> Result<()> {
    let dirs = ProjectDirs::from("ie", "liri", SERVICE_NAME)
        .context("failed to get {SERVICE_NAME} directories")?;
    let data = dirs.data_local_dir();
    std::fs::create_dir_all(data)
        .with_context(|| format!("failed to create project directory {data:?}"))?;
    let log = data.join("log.txt");

    simple_logging::log_to_file(&log, log::LevelFilter::Info)
        .with_context(|| format!("failed to log into {log:?}"))
}
