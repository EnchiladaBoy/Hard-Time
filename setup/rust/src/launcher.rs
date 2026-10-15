#![windows_subsystem = "windows"]
#![forbid(unsafe_code)]

fn main() {
    let result = std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|exe| {
            hardtime_setup::install::launch(
                exe.parent()
                    .ok_or_else(|| anyhow::anyhow!("The game folder could not be found."))?,
            )
        });
    if let Err(error) = result {
        hardtime_setup::platform::message(&format!("{error:#}"));
        std::process::exit(1);
    }
}
