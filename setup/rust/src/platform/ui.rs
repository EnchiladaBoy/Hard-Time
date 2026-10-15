use super::{shell, wide};
use crate::{
    Cancellation,
    install::{self, Installed, Options, Renderer},
};
use anyhow::{Context, Result};
use std::{
    cell::RefCell,
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread::JoinHandle,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*, HiDpi::GetDpiForSystem, Input::KeyboardAndMouse::EnableWindow,
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const SOURCE: usize = 101;
const DESTINATION: usize = 102;
const DOWNLOAD: usize = 103;
const RENDERER: usize = 104;
const SHORTCUTS: usize = 105;
const INSTALL: usize = 106;
const CANCEL: usize = 107;
const BROWSE_FILE: usize = 108;
const BROWSE_FOLDER: usize = 109;
const BROWSE_DESTINATION: usize = 110;
const STATUS: usize = 111;
const PROGRESS: usize = 112;
const BACKGROUND: COLORREF = COLORREF(0x231b14);
const FIELD: COLORREF = COLORREF(0x392d22);
const WHITE: COLORREF = COLORREF(0xf6f2ee);
const MUTED: COLORREF = COLORREF(0xc9baaa);
const ACCENT: COLORREF = COLORREF(0x5594ed);

enum Event {
    Progress(u8, String),
    Done(std::result::Result<Installed, String>),
}

struct State {
    window: HWND,
    receiver: Option<Receiver<Event>>,
    worker: Option<JoinHandle<()>>,
    cancel: Cancellation,
    finishing: bool,
    installed: Option<Installed>,
}

// Paint can re-enter a command handler synchronously. Keep its immutable GDI
// resources outside the RefCell used by the changing installation state.
struct Visual {
    background: HBRUSH,
    field: HBRUSH,
    fonts: Vec<HFONT>,
}

struct Ui {
    visual: Visual,
    state: RefCell<State>,
}

impl Drop for Visual {
    fn drop(&mut self) {
        // The window and its controls have been destroyed before these resources.
        unsafe {
            for font in &self.fonts {
                let _ = DeleteObject((*font).into());
            }
            let _ = DeleteObject(self.background.into());
            let _ = DeleteObject(self.field.into());
        }
    }
}

pub fn message(text: &str) {
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide(text).as_ptr()),
            w!("Hard Time Setup"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn text(window: HWND) -> OsString {
    unsafe {
        let mut value = vec![0; GetWindowTextLengthW(window).max(0) as usize + 1];
        let count = GetWindowTextW(window, &mut value);
        OsString::from_wide(&value[..count.max(0) as usize])
    }
}

fn set_text(window: HWND, value: impl AsRef<std::ffi::OsStr>) {
    unsafe {
        let _ = SetWindowTextW(window, PCWSTR(wide(value).as_ptr()));
    }
}

impl State {
    fn control(&self, id: usize) -> HWND {
        unsafe { GetDlgItem(Some(self.window), id as i32).unwrap_or_default() }
    }
    fn checked(&self, id: usize) -> bool {
        unsafe {
            SendMessageW(self.control(id), BM_GETCHECK, None, None).0 == BST_CHECKED.0 as isize
        }
    }
    fn enabled(&self, id: usize, enabled: bool) {
        unsafe {
            let _ = EnableWindow(self.control(id), enabled);
        }
    }
    fn status(&self, value: &str) {
        set_text(self.control(STATUS), value);
    }
    fn input_state(&self, enabled: bool) {
        for id in [
            SOURCE,
            DESTINATION,
            DOWNLOAD,
            RENDERER,
            SHORTCUTS,
            BROWSE_FILE,
            BROWSE_FOLDER,
            BROWSE_DESTINATION,
        ] {
            self.enabled(id, enabled);
        }
        for id in [SOURCE, BROWSE_FILE, BROWSE_FOLDER] {
            self.enabled(id, enabled && !self.checked(DOWNLOAD));
        }
    }
    fn error(&self, value: &str) {
        unsafe {
            MessageBoxW(
                Some(self.window),
                PCWSTR(wide(value).as_ptr()),
                w!("Hard Time Setup"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
    fn close(&self) {
        if self.receiver.is_some() {
            if !self.finishing {
                self.cancel.cancel();
                self.status("Cancelling safely... Waiting for the current operation to finish.");
                self.enabled(CANCEL, false);
            }
        } else {
            unsafe {
                let _ = DestroyWindow(self.window);
            }
        }
    }
    fn command(&mut self, id: usize) -> Result<()> {
        match id {
            CANCEL => self.close(),
            DOWNLOAD if self.receiver.is_none() && self.installed.is_none() => {
                self.input_state(true)
            }
            BROWSE_FILE | BROWSE_FOLDER | BROWSE_DESTINATION if self.receiver.is_none() => {
                let title = if id == BROWSE_DESTINATION {
                    "Choose a separate empty folder or an existing native installation"
                } else {
                    "Choose the original Hard Time installer or game folder"
                };
                if let Some(path) = shell::pick(self.window, id != BROWSE_FILE, title)? {
                    set_text(
                        self.control(if id == BROWSE_DESTINATION {
                            DESTINATION
                        } else {
                            SOURCE
                        }),
                        path,
                    );
                }
            }
            INSTALL if self.receiver.is_none() => self.start()?,
            _ => (),
        }
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        if let Some(installed) = &self.installed {
            std::process::Command::new(installed.destination.join("Play Hard Time.exe"))
                .current_dir(&installed.destination).spawn().context("The game launcher could not start. Open Play Hard Time.exe in the installation folder.")?;
            self.close();
            return Ok(());
        }
        let source = text(self.control(SOURCE));
        let destination = text(self.control(DESTINATION));
        if (!self.checked(DOWNLOAD) && source.is_empty()) || destination.is_empty() {
            self.status("Choose your original game and an installation folder first.");
            return Ok(());
        }
        let selected = unsafe { SendMessageW(self.control(RENDERER), CB_GETCURSEL, None, None).0 };
        let options = Options {
            source: (!self.checked(DOWNLOAD)).then(|| PathBuf::from(source)),
            destination: destination.into(),
            shortcuts: self.checked(SHORTCUTS),
            renderer: match selected {
                1 => Renderer::Gpu,
                2 => Renderer::Software,
                _ => Renderer::Auto,
            },
        };
        let executable = std::env::current_exe()?;
        self.cancel = Cancellation::default();
        let cancel = self.cancel.clone();
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(std::thread::Builder::new().name("install".into()).spawn(
            move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    install::install(options, &executable, &cancel, |value, text| {
                        let _ = sender.send(Event::Progress(value, text.into()));
                    })
                }))
                .map_err(|_| {
                    "Setup stopped unexpectedly. Run it again to recover the installation."
                        .to_owned()
                })
                .and_then(|result| result.map_err(|error| format!("{error:#}")));
                let _ = sender.send(Event::Done(result));
            },
        )?);
        self.receiver = Some(receiver);
        self.finishing = false;
        self.input_state(false);
        self.enabled(INSTALL, false);
        self.enabled(CANCEL, true);
        set_text(self.control(CANCEL), "Cancel");
        self.status("Preparing installation...");
        Ok(())
    }
    fn tick(&mut self) {
        while let Some(event) = self.receiver.as_ref().and_then(|r| r.try_recv().ok()) {
            match event {
                Event::Progress(value, status) => {
                    unsafe {
                        SendMessageW(
                            self.control(PROGRESS),
                            PBM_SETPOS,
                            Some(WPARAM(value.into())),
                            None,
                        );
                    }
                    if value >= 90 {
                        self.finishing = true;
                        self.enabled(CANCEL, false);
                    }
                    if self.cancel.check().is_ok() || self.finishing {
                        self.status(&status);
                    }
                }
                Event::Done(result) => {
                    self.receiver = None;
                    if let Some(worker) = self.worker.take() {
                        let _ = worker.join();
                    }
                    self.enabled(INSTALL, true);
                    self.enabled(CANCEL, true);
                    set_text(self.control(CANCEL), "Close");
                    match result {
                        Ok(installed) => {
                            self.status(installed.warning.as_deref().unwrap_or(if installed.renderer == "software" {
                                "Ready to play. Compatibility graphics enabled; crowded scenes may run slowly."
                            } else { "Ready to play with your graphics driver." }));
                            set_text(self.control(INSTALL), "Play Hard Time");
                            self.installed = Some(installed);
                        }
                        Err(error) => {
                            unsafe {
                                SendMessageW(
                                    self.control(PROGRESS),
                                    PBM_SETPOS,
                                    Some(WPARAM(0)),
                                    None,
                                );
                            }
                            self.input_state(true);
                            if error.ends_with("Setup cancelled. No update was committed.") {
                                self.status(
                                    "Setup cancelled. You can adjust your choices and try again.",
                                );
                            } else {
                                self.status(
                                    "Setup needs attention. Adjust your choices and try again.",
                                );
                                self.error(&error);
                            }
                        }
                    }
                }
            }
        }
    }
}

// Windows owns message parameters only for the duration of this callback. State
// is boxed by show_setup and is alive until after the message loop. RefCell's
// non-panicking borrow handles synchronous re-entry from native controls.
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_DESTROY {
            PostQuitMessage(0);
            return LRESULT(0);
        }
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            return DefWindowProcW(window, message, wparam, lparam);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Ui;
        if !pointer.is_null() {
            if matches!(
                message,
                WM_CTLCOLORSTATIC
                    | WM_CTLCOLORBTN
                    | WM_CTLCOLOREDIT
                    | WM_CTLCOLORLISTBOX
                    | WM_ERASEBKGND
                    | WM_DRAWITEM
            ) {
                return paint(window, &(*pointer).visual, message, wparam, lparam);
            } else if let Ok(mut state) = (*pointer).state.try_borrow_mut() {
                match message {
                    WM_COMMAND if (wparam.0 >> 16) == 0 => {
                        if let Err(error) = state.command(wparam.0 & 0xffff) {
                            state.error(&format!("{error:#}"));
                        }
                        return LRESULT(0);
                    }
                    WM_CLOSE => {
                        state.close();
                        return LRESULT(0);
                    }
                    WM_TIMER => {
                        state.tick();
                        return LRESULT(0);
                    }
                    _ => (),
                }
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

unsafe fn paint(
    window: HWND,
    state: &Visual,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_ERASEBKGND {
            let mut rect = RECT::default();
            let _ = GetClientRect(window, &mut rect);
            FillRect(HDC(wparam.0 as *mut _), &rect, state.background);
            return LRESULT(1);
        }
        if message == WM_DRAWITEM {
            let item = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let primary = item.CtlID == INSTALL as u32;
            let disabled = (item.itemState.0 & ODS_DISABLED.0) != 0;
            let brush = CreateSolidBrush(if primary && !disabled { ACCENT } else { FIELD });
            FillRect(item.hDC, &item.rcItem, brush);
            let _ = DeleteObject(brush.into());
            SetBkMode(item.hDC, TRANSPARENT);
            SetTextColor(
                item.hDC,
                if disabled {
                    MUTED
                } else if primary {
                    BACKGROUND
                } else {
                    WHITE
                },
            );
            let old = SelectObject(item.hDC, state.fonts[0].into());
            let mut rect = item.rcItem;
            let mut value = wide(text(item.hwndItem));
            value.pop();
            DrawTextW(
                item.hDC,
                &mut value,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(item.hDC, old);
            if item.itemState.0 & ODS_FOCUS.0 != 0 {
                let _ = DrawFocusRect(item.hDC, &item.rcItem);
            }
            return LRESULT(1);
        }
        let dc = HDC(wparam.0 as *mut _);
        let id = GetDlgCtrlID(HWND(lparam.0 as *mut _));
        let field = matches!(message, WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX)
            || matches!(id as usize, SOURCE | DESTINATION);
        SetTextColor(
            dc,
            if id == 202 {
                ACCENT
            } else if matches!(id, 203 | 204 | 205 | 111) {
                MUTED
            } else {
                WHITE
            },
        );
        SetBkColor(dc, if field { FIELD } else { BACKGROUND });
        LRESULT(if field {
            state.field.0
        } else {
            state.background.0
        } as isize)
    }
}

struct Builder {
    window: HWND,
    font: HFONT,
    scale: f64,
    instance: HINSTANCE,
}
impl Builder {
    fn add(
        &self,
        class: PCWSTR,
        value: &str,
        id: usize,
        style: WINDOW_STYLE,
        rect: [i32; 4],
    ) -> Result<HWND> {
        let [x, y, width, height] = rect.map(|v| (f64::from(v) * self.scale).round() as i32);
        unsafe {
            let control = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                PCWSTR(wide(value).as_ptr()),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                width,
                height,
                Some(self.window),
                Some(HMENU(id as *mut _)),
                Some(self.instance),
                None,
            )?;
            SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                Some(LPARAM(1)),
            );
            Ok(control)
        }
    }
    fn label(&self, value: &str, id: usize, rect: [i32; 4]) -> Result<HWND> {
        self.add(w!("STATIC"), value, id, WINDOW_STYLE(0), rect)
    }
    fn button(&self, value: &str, id: usize, rect: [i32; 4]) -> Result<HWND> {
        self.add(
            w!("BUTTON"),
            value,
            id,
            WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            rect,
        )
    }
}

pub fn show_setup() -> Result<()> {
    let _apartment = shell::Apartment::new()?;
    // All handles stay on this thread. Only plain progress messages cross threads.
    unsafe {
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS,
        })
        .ok()?;
        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let class = w!("HardTimeRustSetup");
        // MAKEINTRESOURCE(1): this integer identifies a resource, not memory.
        let icon = LoadIconW(Some(instance), PCWSTR(std::ptr::without_provenance(1)))?;
        let registration = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: icon,
            lpszClassName: class,
            ..Default::default()
        };
        anyhow::ensure!(
            RegisterClassW(&registration) != 0,
            "The setup window could not be registered."
        );
        let mut area = RECT::default();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut area as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
        let scale = (f64::from(GetDpiForSystem()) / 96.0)
            .min(f64::from(area.right - area.left - 40) / 700.0)
            .min(f64::from(area.bottom - area.top - 60) / 580.0);
        let mut rect = RECT {
            right: (700.0 * scale) as i32,
            bottom: (580.0 * scale) as i32,
            ..Default::default()
        };
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        AdjustWindowRectEx(&mut rect, style, false, WS_EX_CONTROLPARENT)?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class,
            w!("Hard Time Setup"),
            style,
            area.left + (area.right - area.left - width) / 2,
            area.top + (area.bottom - area.top - height) / 2,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )?;
        let font = |height: i32, weight: i32| {
            CreateFontW(
                -(f64::from(height) * scale).round() as i32,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            )
        };
        let fonts = vec![
            font(14, FW_NORMAL.0 as i32),
            font(38, FW_BOLD.0 as i32),
            font(14, FW_BOLD.0 as i32),
        ];
        let builder = Builder {
            window,
            font: fonts[0],
            scale,
            instance,
        };
        let title = builder.label("HARD TIME", 201, [28, 18, 640, 55])?;
        SendMessageW(
            title,
            WM_SETFONT,
            Some(WPARAM(fonts[1].0 as usize)),
            Some(LPARAM(1)),
        );
        let tagline = builder.label(
            "THE ORIGINAL GAME. READY FOR MODERN WINDOWS.",
            202,
            [30, 77, 640, 25],
        )?;
        SendMessageW(
            tagline,
            WM_SETFONT,
            Some(WPARAM(fonts[2].0 as usize)),
            Some(LPARAM(1)),
        );
        builder.label(
            "Download, install, play. Setup handles the rest.",
            203,
            [30, 113, 640, 25],
        )?;
        builder.label("1   Original game", 206, [30, 153, 170, 25])?;
        let download = builder.add(
            w!("BUTTON"),
            "Download from MDickie automatically",
            DOWNLOAD,
            WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            [205, 148, 465, 30],
        )?;
        let source = builder.add(
            w!("EDIT"),
            "",
            SOURCE,
            WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            [30, 185, 424, 29],
        )?;
        builder.button("Installer...", BROWSE_FILE, [464, 183, 98, 32])?;
        builder.button("Folder...", BROWSE_FOLDER, [572, 183, 98, 32])?;
        builder.label(
            "Original game by Mat Dickie / MDickie.com",
            204,
            [30, 224, 640, 23],
        )?;
        builder.label("2   Install to", 207, [30, 260, 640, 25])?;
        let destination = builder.add(
            w!("EDIT"),
            "",
            DESTINATION,
            WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            [30, 291, 532, 29],
        )?;
        set_text(destination, shell::default_destination()?);
        builder.button("Change...", BROWSE_DESTINATION, [572, 289, 98, 32])?;
        builder.label("Graphics", 208, [30, 338, 80, 25])?;
        let renderer = builder.add(
            w!("COMBOBOX"),
            "",
            RENDERER,
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            [112, 334, 400, 150],
        )?;
        for name in [
            "Automatic (recommended)",
            "GPU driver",
            "Compatibility (software rendering)",
        ] {
            SendMessageW(
                renderer,
                CB_ADDSTRING,
                None,
                Some(LPARAM(wide(name).as_ptr() as isize)),
            );
        }
        SendMessageW(renderer, CB_SETCURSEL, Some(WPARAM(0)), None);
        let shortcuts = builder.add(
            w!("BUTTON"),
            "Create Desktop and Start menu shortcuts",
            SHORTCUTS,
            WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            [30, 379, 640, 27],
        )?;
        // Classic checkbox text honors WM_CTLCOLORSTATIC; themed checkbox text
        // otherwise remains black on the dark installer background.
        SetWindowTheme(download, w!(""), w!(""))?;
        SetWindowTheme(shortcuts, w!(""), w!(""))?;
        SendMessageW(
            shortcuts,
            BM_SETCHECK,
            Some(WPARAM(BST_CHECKED.0 as usize)),
            None,
        );
        builder.add(
            PROGRESS_CLASSW,
            "",
            PROGRESS,
            WINDOW_STYLE(0),
            [30, 423, 640, 7],
        )?;
        builder.label(
            "Your original files and existing saves are kept.",
            STATUS,
            [30, 440, 640, 56],
        )?;
        builder.button("Install game", INSTALL, [30, 508, 400, 36])?;
        builder.button("Close", CANCEL, [450, 508, 220, 36])?;
        builder.label(
            "Unofficial edition  /  Hard Time © Mat Dickie 2007",
            205,
            [30, 556, 640, 20],
        )?;
        if let Some(candidate) = shell::original_candidate() {
            set_text(source, candidate);
        } else {
            SendMessageW(
                download,
                BM_SETCHECK,
                Some(WPARAM(BST_CHECKED.0 as usize)),
                None,
            );
        }
        let state = Box::new(Ui {
            visual: Visual {
                background: CreateSolidBrush(BACKGROUND),
                field: CreateSolidBrush(FIELD),
                fonts,
            },
            state: RefCell::new(State {
                window,
                receiver: None,
                worker: None,
                cancel: Cancellation::default(),
                finishing: false,
                installed: None,
            }),
        });
        SetWindowLongPtrW(window, GWLP_USERDATA, (&*state as *const Ui) as isize);
        state.state.borrow().input_state(true);
        SetTimer(Some(window), 1, 100, None);
        let _ = ShowWindow(window, SW_SHOW);
        let _ = UpdateWindow(window);
        let mut event = MSG::default();
        loop {
            let result = GetMessageW(&mut event, None, 0, 0).0;
            if result <= 0 {
                break;
            }
            if !IsDialogMessageW(window, &event).as_bool() {
                let _ = TranslateMessage(&event);
                DispatchMessageW(&event);
            }
        }
        let _ = UnregisterClassW(class, Some(instance));
        drop(state);
    }
    Ok(())
}
