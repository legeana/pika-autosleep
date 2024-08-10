use clap::{Parser, Subcommand};

pub const SERVICE_COMMAND: &str = "service";

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Install,
    Uninstall,
    Service,
}

pub fn parse() -> Cli {
    Cli::parse()
}
