use anyhow::{Context, Result, ensure};
use std::ffi::CStr;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Gdi::*, OpenGL::*},
        System::LibraryLoader::*,
        UI::WindowsAndMessaging::*,
    },
    core::{s, w},
};

type Create = unsafe extern "system" fn(HDC) -> HGLRC;
type Current = unsafe extern "system" fn(HDC, HGLRC) -> i32;
type Delete = unsafe extern "system" fn(HGLRC) -> i32;
type StringFn = unsafe extern "system" fn(u32) -> *const u8;

pub fn graphics_available() -> bool {
    probe().unwrap_or(false)
}

fn probe() -> Result<bool> {
    // Called only in the short-lived --probe-graphics child. A driver failure
    // cannot crash setup. Load system OpenGL explicitly, ignoring app-local Mesa.
    unsafe {
        let module = LoadLibraryExW(w!("opengl32.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)?;
        struct Library(HMODULE);
        impl Drop for Library {
            fn drop(&mut self) {
                unsafe {
                    let _ = FreeLibrary(self.0);
                }
            }
        }
        let _library = Library(module);
        let create: Create =
            std::mem::transmute(GetProcAddress(module, s!("wglCreateContext")).context("No WGL")?);
        let current: Current =
            std::mem::transmute(GetProcAddress(module, s!("wglMakeCurrent")).context("No WGL")?);
        let delete: Delete =
            std::mem::transmute(GetProcAddress(module, s!("wglDeleteContext")).context("No WGL")?);
        let string: StringFn =
            std::mem::transmute(GetProcAddress(module, s!("glGetString")).context("No GL")?);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Hard Time graphics check"),
            WS_POPUP,
            0,
            0,
            16,
            16,
            None,
            None,
            None,
            None,
        )?;
        let dc = GetDC(Some(window));
        let mut context = HGLRC::default();
        let result = (|| -> Result<bool> {
            ensure!(!dc.is_invalid(), "No graphics device context");
            let format = PIXELFORMATDESCRIPTOR {
                nSize: size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cDepthBits: 24,
                cStencilBits: 8,
                ..Default::default()
            };
            let index = ChoosePixelFormat(dc, &format);
            ensure!(index != 0, "No OpenGL pixel format");
            SetPixelFormat(dc, index, &format)?;
            context = create(dc);
            ensure!(
                !context.is_invalid() && current(dc, context) != 0,
                "No OpenGL context"
            );
            let pointer = string(GL_VERSION);
            ensure!(!pointer.is_null(), "No OpenGL version");
            let version = CStr::from_ptr(pointer.cast())
                .to_str()?
                .split_whitespace()
                .next()
                .unwrap_or("");
            let mut parts = version.split('.');
            let major: u32 = parts.next().unwrap_or("").parse()?;
            let minor: u32 = parts.next().unwrap_or("").parse()?;
            Ok((major, minor) >= (3, 3))
        })();
        current(HDC::default(), HGLRC::default());
        if !context.is_invalid() {
            delete(context);
        }
        ReleaseDC(Some(window), dc);
        let _ = DestroyWindow(window);
        result
    }
}
