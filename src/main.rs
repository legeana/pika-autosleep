use anyhow::Result;

mod cli;
mod install;
mod service;

fn main() -> Result<()> {
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
