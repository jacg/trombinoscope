use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Directory containing the class assets
    pub class_dir: PathBuf,

    #[arg(long)]
    pub strip_metadata: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
