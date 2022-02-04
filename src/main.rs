/// floki - the development container launcher
#[macro_use]
extern crate log;

mod cli;
mod command;
mod config;
mod dind;
mod environment;
mod errors;
mod image;
mod interpret;
mod spec;
mod volumes;

use anyhow::Error;
use cli::{Cli, Subcommand};
use config::FlokiConfig;
use environment::Environment;
use structopt::StructOpt;

use std::path::Path;
use std::fs;
use std::env;

fn main() -> Result<(), Error> {
    let args = Cli::from_args();
    configure_logging(args.verbosity)?;

    match run_floki_from_args(&args) {
        Ok(()) => (),
        Err(e) => {
            error!("A problem occurred: {}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Decide which commands to run given the input from the shell
fn run_floki_from_args(args: &Cli) -> Result<(), Error> {
    debug!("Got command line arguments: {:?}", &args);

    if args.local {
        warn!("-l/--local is deprecated and may be removed in a future release");
    }


    // Check whether a floki config file exists, if so, load the extra command
    // and append to the inner command.

    // Dispatch appropriate subcommand
    match &args.subcommand {
        // Pull the image in the configuration file
        Some(Subcommand::Pull {}) => {
            let env = Environment::gather(&args.config_file)?;
            let config = FlokiConfig::from_file(&env.config_file)?;
            image::pull_image(&config.image.name()?)
        }

        // Run a command in the floki container
        Some(Subcommand::Run { command }) => {
            let env = Environment::gather(&args.config_file)?;
            let config = FlokiConfig::from_file(&env.config_file)?;
            let inner_command = interpret::command_in_shell(config.shell.inner_shell(), command);
            let inner_command = append_global_config(inner_command.as_str());
            interpret::run_floki_container(&spec::FlokiSpec::from(config, env)?, &inner_command)
        }

        Some(Subcommand::Completion { shell }) => {
            Cli::clap().gen_completions_to("floki", *shell, &mut std::io::stdout());
            Ok(())
        }

        // Launch an interactive floki shell (the default)
        None => {
            let env = Environment::gather(&args.config_file)?;
            let config = FlokiConfig::from_file(&env.config_file)?;
            let inner_command = config.shell.inner_shell();
            let inner_command = append_global_config(inner_command);
            interpret::run_floki_container(&spec::FlokiSpec::from(config, env)?, &inner_command)
        }
    }
}

/// Searches for a startup script in $HOME/.floki, if found, will run commands
/// in the floki container when the container starts up.
fn append_global_config(command: &str) -> String {
    // Retrieve the name of the home directory.
    let home = match env::var("HOME") {
        Ok(val) => val,
        Err(_) => return command.to_string(),
    };

    // Check for the existence of the startup script.
    let filepath = format!("{}/.floki/startup.sh", home.as_str());
    let startup_script_exists = Path::new(&filepath).exists();
    if startup_script_exists {
        let contents = match fs::read_to_string(filepath) {
            Ok(data) => data,
            Err(_) => return command.to_string(),
        };
        return format!("{} && {}", contents.trim(), command)
    }

    command.to_string()
}

/// Configure the logger
fn configure_logging(verbosity: u8) -> Result<(), Error> {
    let level = match verbosity {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        3 => log::LevelFilter::Trace,
        _ => {
            return Err(
                errors::FlokiUserError::InvalidVerbositySetting { setting: verbosity }.into(),
            )
        }
    };
    simplelog::TermLogger::init(
        level,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stderr,
        simplelog::ColorChoice::Auto,
    )?;
    Ok(())
}
