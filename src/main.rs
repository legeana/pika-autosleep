use anyhow::Result;

mod cli;
mod install;
mod service;

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .default_format()
        .init();
    let args = cli::parse();
    match args.command {
        cli::Command::Install => {
            install::install()?;
        }
        cli::Command::Uninstall => {
            install::uninstall()?;
        }
        cli::Command::Service => {
            service::start()?;
        }
    }
    Ok(())
}
