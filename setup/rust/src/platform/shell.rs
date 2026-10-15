use super::wide;
use crate::files;
use anyhow::{Result, ensure};
use std::{
    ffi::OsString,
    fs,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::*,
        UI::Shell::{Common::COMDLG_FILTERSPEC, *},
    },
    core::{GUID, Interface, PCWSTR, w},
};

pub(super) struct Apartment;
impl Apartment {
    pub(super) fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn known_folder(id: &GUID) -> Result<PathBuf> {
    unsafe {
        let value = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)?;
        let result = PathBuf::from(OsString::from_wide(value.as_wide()));
        CoTaskMemFree(Some(value.0.cast()));
        Ok(result)
    }
}

pub fn default_destination() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_LocalAppData)?.join("Programs/Hard Time"))
}

pub fn original_candidate() -> Option<PathBuf> {
    known_folder(&FOLDERID_Downloads)
        .ok()
        .map(|p| p.join("hardtime_setup.exe"))
        .filter(|p| p.is_file())
}

pub(super) fn pick(owner: HWND, folder: bool, title: &str) -> Result<Option<PathBuf>> {
    // The UI thread owns a COM apartment for the lifetime of its message loop.
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let options = FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR;
        dialog.SetOptions(if folder {
            options | FOS_PICKFOLDERS
        } else {
            options | FOS_FILEMUSTEXIST
        })?;
        dialog.SetTitle(PCWSTR(wide(title).as_ptr()))?;
        if !folder {
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("Original installer (*.exe)"),
                pszSpec: w!("*.exe"),
            }])?;
            dialog.SetFileName(w!("hardtime_setup.exe"))?;
        }
        if let Err(error) = dialog.Show(Some(owner)) {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error.into());
        }
        let value = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let result = PathBuf::from(OsString::from_wide(value.as_wide()));
        CoTaskMemFree(Some(value.0.cast()));
        Ok(Some(result))
    }
}

pub fn create_shortcuts(root: &Path, folders: Option<&[PathBuf]>) -> Result<()> {
    let _apartment = Apartment::new()?;
    let defaults;
    let folders = match folders {
        Some(value) => value,
        None => {
            defaults = [
                known_folder(&FOLDERID_Desktop)?,
                known_folder(&FOLDERID_Programs)?,
            ];
            &defaults
        }
    };
    let target = files::absolute(&root.join("Play Hard Time.exe"))?;
    for folder in folders {
        let path = folder.join("Hard Time (Native).lnk");
        files::private_path(&path)?;
        fs::create_dir_all(folder)?;
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let persist: IPersistFile = link.cast()?;
            if path.exists() {
                persist.Load(PCWSTR(wide(&path).as_ptr()), STGM_READ)?;
                let mut previous = [0_u16; 32768];
                link.GetPath(&mut previous, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)?;
                let end = previous
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(previous.len());
                let previous = PathBuf::from(OsString::from_wide(&previous[..end]));
                ensure!(
                    files::within(&target, &previous) && files::within(&previous, &target),
                    "A shortcut already belongs to a different installation."
                );
            }
            link.SetPath(PCWSTR(wide(&target).as_ptr()))?;
            link.SetWorkingDirectory(PCWSTR(wide(root).as_ptr()))?;
            link.SetDescription(w!("Hard Time - native Windows edition"))?;
            // Replace atomically so a failed save cannot damage an old shortcut.
            let temporary = tempfile::Builder::new()
                .prefix(".hardtime-shortcut-")
                .tempdir_in(folder)?;
            let staged = temporary.path().join("Play.lnk");
            persist.Save(PCWSTR(wide(&staged).as_ptr()), true)?;
            fs::rename(staged, path)?;
        }
    }
    Ok(())
}
