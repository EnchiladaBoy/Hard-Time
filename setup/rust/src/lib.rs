#![deny(unsafe_code)]

pub mod files;
pub mod install;
pub mod payload;
pub mod transaction;

// Win32 requires FFI. All unsafe code is confined to this module; the installer,
// recovery journal, archive verification and launcher use safe Rust.
#[allow(unsafe_code)]
pub mod platform;

use anyhow::{Result, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<()> {
        ensure!(
            !self.0.load(Ordering::Relaxed),
            "Setup cancelled. No update was committed."
        );
        Ok(())
    }
}
