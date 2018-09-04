//! CLI
//!
//! Tooling related to command line interface usage.

use std::path::PathBuf;
use structopt::StructOpt;

/// Saha Language Interpreter
#[derive(StructOpt, Debug)]
#[structopt(name = "saha")]
pub struct InterpreterArgs {
    /// Saha entrypoint file, containing a `main()` function
    #[structopt(name = "FILE", parse(from_os_str))]
    pub entrypoint: PathBuf
}

/// Get command line arguments given to the interpreter.
pub fn get_cli_arguments() -> InterpreterArgs {
    return InterpreterArgs::from_args();
}

#[cfg(test)]
mod tests {
    // TODO
}
