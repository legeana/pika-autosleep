use anyhow::Result;

mod cli;
mod install;
mod logconfig;
mod service;

fn wrap_errors() -> Result<()> {
    let args = cli::parse();
    match args.command {
        cli::Command::Install => {
            logconfig::init_cli()?;
            install::install()?;
        }
        cli::Command::Uninstall => {
            logconfig::init_cli()?;
            install::uninstall()?;
        }
        cli::Command::Service => {
            logconfig::init_service()?;
            service::start()?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = wrap_errors() {
        log::error!("failed: {err}");
    };
}
