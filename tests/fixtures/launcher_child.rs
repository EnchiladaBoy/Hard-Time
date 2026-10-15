//! Disposable stand-in for the game used by packaged-launcher integration tests.
use std::{env, fs, path::PathBuf, thread, time::{Duration, Instant}};

fn main() {
    println!("cwd={}", env::current_dir().unwrap().display());
    println!("renderer={}", env::var("GALLIUM_DRIVER").unwrap_or_default());
    eprintln!("launcher diagnostic fixture");
    if let Some(started) = env::var_os("HARDTIME_TEST_STARTED") {
        fs::write(started, b"started").unwrap();
    }
    if let Some(release) = env::var_os("HARDTIME_TEST_RELEASE") {
        let release = PathBuf::from(release);
        let start = Instant::now();
        while !release.exists() && start.elapsed() < Duration::from_secs(60) {
            thread::sleep(Duration::from_millis(50));
        }
    }
}
