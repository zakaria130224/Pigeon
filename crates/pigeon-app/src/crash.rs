//! Keeps a record if the app ever crashes, so a problem can be reported and fixed.

use pigeon_core::{VERSION, settings};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn install() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let folder = settings::app_dir();
        let _ = fs::create_dir_all(&folder);
        if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(folder.join("crash.log")) {
            let when = chrono::Local::now().to_rfc3339();
            let backtrace = std::backtrace::Backtrace::force_capture();
            let _ = writeln!(log, "--- {when} Pigeon {VERSION} on {}\n{info}\n{backtrace}", std::env::consts::OS);
        }
        default_hook(info);
    }));
}
