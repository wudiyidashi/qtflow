use clap::Parser;
use qtflow::{app, cli::Cli, error::QtflowError};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let exit_code = match err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => QtflowError::ConfigOrArg(err.to_string()).exit_code(),
            };
            let _ = err.print();
            std::process::exit(exit_code);
        }
    };

    if let Err(err) = app::run(cli) {
        if !err.already_reported() {
            eprintln!("{err}");
        }
        std::process::exit(err.exit_code());
    }
}
