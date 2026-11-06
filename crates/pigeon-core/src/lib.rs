//! Everything in Pigeon that doesn't need a window: the data model, the workspace on disk,
//! sending requests, scripts, importing and exporting, and git team sync.

pub mod formats;
pub mod git;
pub mod http;
pub mod model;
pub mod scripting;
pub mod settings;
pub mod suggest;
pub mod workspace;

/// The product name, also used for its folders on disk.
pub const APP_NAME: &str = "Pigeon";

/// The version of this build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
