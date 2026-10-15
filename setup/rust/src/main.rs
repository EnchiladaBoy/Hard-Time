#![windows_subsystem = "windows"]
#![forbid(unsafe_code)]

use anyhow::{Context, Result, bail, ensure};
use hardtime_setup::{
    Cancellation, files,
    install::{self, Options, Renderer},
    platform,
};
use std::{ffi::OsString, path::PathBuf};

const USAGE: &str = "Hard Time Rust Setup\n\nOpen without arguments for the installer window.\n\n--install (--source PATH | --download-original) --destination PATH\n  [--renderer auto|gpu|software] [--no-shortcuts] [--result FILE]\n\nPlayers do not need Rust, .NET, Python or a separate 7-Zip installation.";

fn command_line(args: Vec<OsString>, result: &mut Option<PathBuf>) -> Result<()> {
    let mut source = None;
    let mut destination = None;
    let mut renderer = Renderer::Auto;
    let mut shortcuts = true;
    let mut install = false;
    let mut download = false;
    let mut requested_report = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--install") => install = true,
            Some("--download-original") => download = true,
            Some("--no-shortcuts") => shortcuts = false,
            Some("--source") => {
                source = Some(PathBuf::from(args.next().context("--source needs a path")?))
            }
            Some("--destination") => {
                destination = Some(PathBuf::from(
                    args.next().context("--destination needs a path")?,
                ))
            }
            Some("--renderer") => {
                renderer = args
                    .next()
                    .context("--renderer needs a choice")?
                    .to_str()
                    .context("Invalid renderer")?
                    .parse()?
            }
            Some("--result") => {
                requested_report = Some(files::absolute(&PathBuf::from(
                    args.next().context("--result needs a path")?,
                ))?)
            }
            _ => bail!("Unknown argument: {}\n{USAGE}", arg.to_string_lossy()),
        }
    }
    ensure!(
        install && destination.is_some() && download != source.is_some(),
        "{USAGE}"
    );
    // Reject a result file inside the installation or the source: an automation
    // report must never overwrite player data, a payload or the input installer.
    if let Some(report) = requested_report.as_ref() {
        files::private_path(report)?;
        let dest = files::absolute(destination.as_ref().unwrap())?;
        ensure!(
            !files::within(report, &dest),
            "Choose a result file outside the installation."
        );
        if let Some(input) = &source {
            ensure!(
                !files::within(report, &files::absolute(input)?),
                "Choose a result file outside the original game."
            );
        }
        let executable = std::env::current_exe()?;
        ensure!(
            !(files::within(report, &executable) && files::within(&executable, report)),
            "The result file cannot replace setup."
        );
    }
    *result = requested_report;
    let installed = install::install(
        Options {
            source,
            destination: destination.unwrap(),
            renderer,
            shortcuts,
        },
        &std::env::current_exe()?,
        &Cancellation::default(),
        |_, _| {},
    )?;
    if let Some(path) = result {
        write_result(path, &serde_json::to_vec_pretty(&installed)?)?;
    }
    Ok(())
}

fn write_result(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    files::private_path(path)?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("Invalid result path")?)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--probe-graphics"] {
        std::process::exit(if platform::graphics_available() { 0 } else { 1 });
    }
    if args == ["--help"] {
        println!("{USAGE}");
        return;
    }
    if args.is_empty() {
        if let Err(error) = platform::show_setup() {
            platform::message(&format!("{error:#}"));
            std::process::exit(1);
        }
        return;
    }
    let mut report = None;
    if let Err(error) = command_line(args, &mut report) {
        eprintln!("{error:#}");
        // Only publish to a validated destination; invalid argument/report paths
        // are reported on stderr without touching the requested path.
        if let Some(path) = report {
            let _ = write_result(
                &path,
                &serde_json::to_vec_pretty(&serde_json::json!({"error": format!("{error:#}")}))
                    .unwrap(),
            );
        }
        std::process::exit(1);
    }
}
