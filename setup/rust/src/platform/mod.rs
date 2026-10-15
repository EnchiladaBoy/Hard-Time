//! Windows APIs are confined here. File copying, journals and installation
//! decisions live in safe Rust and are exercised separately from the UI.
mod graphics;
mod network;
mod shell;
mod ui;

pub use graphics::graphics_available;
pub use network::download_original;
pub use shell::{create_shortcuts, default_destination, original_candidate};
pub use ui::{message, show_setup};

use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
