use std::fs::File;

use anyhow::{anyhow, Context, Result};

pub fn init_cli() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .default_format()
        .try_init()
        .context("failed to build stderr logger")
}

pub fn init_service() -> Result<()> {
    let exe = std::env::current_exe().context("failed to get current executable")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("failed to get {exe:?} parent directory"))?;
    let log = dir.join("service-log.txt");

    let target = Box::new(File::create(&log).with_context(|| format!("failed to open {log:?}"))?);
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .default_format()
        .format_timestamp_secs()
        .target(env_logger::Target::Pipe(target))
        .try_init()
        .with_context(|| format!("failed to build logger into {log:?}"))
}
