use anyhow::{anyhow, Context, Result};

pub fn init_cli() -> Result<()> {
    simple_logging::log_to_stderr(log::LevelFilter::Info);
    Ok(())
}

pub fn init_service() -> Result<()> {
    let exe = std::env::current_exe().context("failed to get current executable")?;
    let dir = exe.parent().ok_or_else(|| anyhow!("failed to get {exe:?} parent directory"))?;
    let log = dir.join("service-log.txt");
    simple_logging::log_to_file(&log, log::LevelFilter::Info)
        .with_context(|| format!("failed to log into {log:?}"))
}
