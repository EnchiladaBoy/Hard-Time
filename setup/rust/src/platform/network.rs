use crate::{Cancellation, files, install::ORIGINAL_HASH};
use anyhow::{Result, ensure};
use std::{
    ffi::c_void,
    fs::File,
    io::Write,
    path::Path,
    ptr,
    time::{Duration, Instant},
};
use windows::{
    Win32::Networking::WinHttp::*,
    core::{PCWSTR, w},
};

struct Internet(*mut c_void);
impl Internet {
    fn new(handle: *mut c_void) -> Result<Self> {
        ensure!(
            !handle.is_null(),
            "Windows could not open the download connection: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(handle))
    }
}
impl Drop for Internet {
    fn drop(&mut self) {
        // Each handle has one owner and outlives its dependent request/connection.
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}

pub fn download_original(
    target: &Path,
    cancel: &Cancellation,
    progress: &mut impl FnMut(u8, &str),
) -> Result<()> {
    cancel.check()?;
    files::private_path(target)?;
    // Synchronous WinHTTP uses Windows certificate validation and proxy settings.
    // A blocked call is bounded by five seconds; cancellation is checked between
    // calls. Redirects may never downgrade HTTPS to plain HTTP.
    unsafe {
        let session = Internet::new(WinHttpOpen(
            w!("Hard-Time-Rust-Setup/0.1"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000)?;
        let connection = Internet::new(WinHttpConnect(session.0, w!("www.mdickie.com"), 443, 0))?;
        let request = Internet::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            w!("/downloads/new/hardtime_setup.exe"),
            PCWSTR::null(),
            PCWSTR::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        WinHttpSetOption(
            Some(request.0),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_ne_bytes()),
        )?;
        cancel.check()?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0)?;
        cancel.check()?;
        WinHttpReceiveResponse(request.0, ptr::null_mut())?;
        let mut status = 0_u32;
        let mut size = size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut size,
            ptr::null_mut(),
        )?;
        ensure!(
            status == 200,
            "MDickie's download returned HTTP {status}. Select a local original copy or try later."
        );
        let mut output = File::create_new(target)?;
        let mut buffer = [0_u8; 65536];
        let mut total = 0_u64;
        let started = Instant::now();
        loop {
            cancel.check()?;
            ensure!(
                started.elapsed() < Duration::from_secs(180),
                "The download timed out. Try a local original copy."
            );
            let mut count = 0;
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut count,
            )?;
            if count == 0 {
                break;
            }
            total += u64::from(count);
            ensure!(
                total <= 32 * 1024 * 1024,
                "The original download is larger than expected. Choose a local original copy."
            );
            output.write_all(&buffer[..count as usize])?;
            progress(
                10 + (total * 14 / 20_591_920).min(14) as u8,
                &format!(
                    "Downloading the original game from MDickie... {} MB",
                    total / 1_048_576
                ),
            );
        }
        output.sync_all()?;
    }
    cancel.check()?;
    ensure!(
        files::hash(target)? == ORIGINAL_HASH,
        "The downloaded original differs from the tested version. Choose a complete original game folder instead."
    );
    Ok(())
}
