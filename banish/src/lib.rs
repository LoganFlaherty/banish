//! Start with the reference's Execution Model section on onward.
//! Add ,ignore to all rust code blocks.

mod banish_dispatch;

pub use banish_derive::{ banish, machine, BanishDispatch };
pub use banish_dispatch::BanishDispatch;
pub use log;
use std::fs::File;

/// Initialises banish's built-in trace logger.
///
/// This is a convenience wrapper around [`env_logger`] that configures trace-level
/// logging for banish without requiring any environment variables or manual logger
/// setup. Call this once at the start of `main` before any `banish!` blocks run.
///
/// # Arguments
///
/// * `file_path` - If `Some`, trace output is written to the file at the given path.
///   The file is created if it does not exist and truncated if it does.
///   If `None`, output is written to stderr.
///
/// # Panics
///
/// Panics if `file_path` is `Some` and the file cannot be created, or if a global
/// logger has already been set by another call to this function or an external crate.
///
/// # Examples
///
/// Print trace output to stderr:
/// ```rust
/// banish::init_trace(None);
/// ```
///
/// Write trace output to a file:
/// ```rust
/// banish::init_trace(Some("trace.log"));
/// ```
///
/// If you need more control over log routing or filtering, skip this function and
/// initialise your own [`log`]-compatible backend instead. Banish emits all trace
/// diagnostics through the [`log`] facade, so any backend will capture them.
#[cfg(feature = "trace-logger")]
pub fn init_trace(file_path: Option<&str>) {
    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Trace);

    if let Some(path) = file_path {
        let file = File::create(path).expect("banish: could not open trace file");
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }

    builder.init();
}