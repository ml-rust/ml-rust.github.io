//! Command line front end for the ml-rust catalogue data files.

pub mod cli;
pub mod data_dir;
pub mod validate;

pub use cli::{Cli, Command, run};
pub use data_dir::{DataDir, DataDirError, read_data_dir};
pub use validate::{Outcome, Summary, validate};
