//! The status window: a layered child of the Windows 11 taskbar, driven by one
//! dedicated thread. Nothing here touches the runtime, so this thread never
//! waits on async work while Explorer's input queue is attached to it.

use std::cell::RefCell;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_FRAME: u32 = WM_APP + 1;
const WM_VISIBLE: u32 = WM_APP + 2;
const RECONCILE_TIMER: usize = 1;
const CLOSE_COMMAND: usize = 1;
const CONTROL_CLASS: &str = "RoutaTaskbarStatusControl";
const STATUS_CLASS: &str = "RoutaTaskbarStatus";
const WIDTH_AT_96_DPI: i32 = 95;
const HEIGHT_AT_96_DPI: i32 = 36;

/// Everything one paint needs.
pub struct Frame;

/// Taskbar-client x of the window: left of the notification area, or the right
/// edge of the taskbar when that area cannot be measured.
pub fn place_x(notify_left: i32, tray_width: i32, width: i32) -> i32 {
    if notify_left > 0 {
        notify_left - width
    } else {
        tray_width - width
    }
}

/// Windows 11 starts at build 22000; RtlGetVersion ignores manifest shims.
pub fn supported() -> bool {
    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    unsafe { RtlGetVersion(&mut info) == 0 && info.dwBuildNumber >= 22000 }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub struct Host {
    control: isize,
    frame: Arc<Mutex<Option<Frame>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Host {
    /// `None` below Windows 11 or when the window thread cannot start.
    pub fn spawn(on_left: Box<dyn Fn() + Send>, on_close: Box<dyn Fn() + Send>) -> Option<Host> {
        if !supported() {
            return None;
        }
        let frame = Arc::new(Mutex::new(None));
        let shared = frame.clone();
        let (ready, control) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("taskbar-status".into())
            .spawn(move || run(on_left, on_close, shared, ready))
            .ok()?;
        let control = control.recv().ok()??;
        Some(Host { control, frame, thread: Mutex::new(Some(thread)) })
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe { PostMessageW(self.control as HWND, WM_VISIBLE, visible as usize, 0) };
    }

    pub fn update(&self, frame: Frame) {
        *self.frame.lock().unwrap() = Some(frame);
        unsafe { PostMessageW(self.control as HWND, WM_FRAME, 0, 0) };
    }

    /// Destroys the window; the bar must not outlive the app on the taskbar.
    pub fn shutdown(&self) {
        unsafe { PostMessageW(self.control as HWND, WM_CLOSE, 0, 0) };
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
            while !thread.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}

struct State {
    control: HWND,
    window: HWND,
    tray: HWND,
    embedded: bool,
    visible: bool,
    bounds: (i32, i32, i32, i32),
    taskbar_created: u32,
    on_left: Box<dyn Fn() + Send>,
    on_close: Box<dyn Fn() + Send>,
    frame: Arc<Mutex<Option<Frame>>>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.try_borrow_mut().ok()?.as_mut().map(f))
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

fn run(
    on_left: Box<dyn Fn() + Send>,
    on_close: Box<dyn Fn() + Send>,
    frame: Arc<Mutex<Option<Frame>>>,
    ready: mpsc::Sender<Option<isize>>,
) {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let (control_class, status_class) = (wide(CONTROL_CLASS), wide(STATUS_CLASS));
        for (class, proc) in [(&control_class, control_proc as WndProc), (&status_class, status_proc as WndProc)] {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(proc);
            wc.hInstance = instance;
            wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
        }
        // A top-level window, because only those receive "TaskbarCreated".
        let control = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            control_class.as_ptr(),
            control_class.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if control.is_null() {
            let _ = ready.send(None);
            return;
        }
        let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        STATE.with(|state| {
            *state.borrow_mut() = Some(State {
                control,
                window: std::ptr::null_mut(),
                tray: std::ptr::null_mut(),
                embedded: false,
                visible: true,
                bounds: (0, 0, 0, 0),
                taskbar_created,
                on_left,
                on_close,
                frame,
            })
        });
        SetTimer(control, RECONCILE_TIMER, 1000, None);
        with_state(reconcile);
        let _ = ready.send(Some(control as isize));
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        STATE.with(|state| *state.borrow_mut() = None);
    }
}

unsafe extern "system" fn control_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = STATE.with(|state| state.try_borrow().ok().and_then(|s| s.as_ref().map(|s| s.taskbar_created)));
    match msg {
        WM_TIMER => {
            with_state(reconcile);
            0
        }
        WM_FRAME => {
            with_state(paint);
            0
        }
        WM_VISIBLE => {
            with_state(|s| {
                s.visible = wparam != 0;
                reconcile(s)
            });
            0
        }
        WM_CLOSE => {
            with_state(|s| {
                if !s.window.is_null() {
                    unsafe { DestroyWindow(s.window) };
                }
                s.window = std::ptr::null_mut();
            });
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ if Some(msg) == taskbar_created => {
            with_state(reconcile);
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn status_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Asking the taskbar (the parent) could eat the click.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_LBUTTONUP => {
            with_state(|s| (s.on_left)());
            0
        }
        // The state borrow is released while the menu's nested loop runs.
        WM_RBUTTONUP => {
            let control = with_state(|s| s.control);
            if control.is_some_and(|c| unsafe { show_menu(c) }) {
                with_state(|s| (s.on_close)());
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe fn show_menu(owner: HWND) -> bool {
    unsafe {
        let menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, CLOSE_COMMAND, wide("关闭").as_ptr());
        let mut at = POINT { x: 0, y: 0 };
        GetCursorPos(&mut at);
        SetForegroundWindow(owner);
        let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, 0, owner, std::ptr::null());
        DestroyMenu(menu);
        PostMessageW(owner, WM_NULL, 0, 0);
        chosen as usize == CLOSE_COMMAND
    }
}

fn find(parent: HWND, class: &str) -> HWND {
    unsafe { FindWindowExW(parent, std::ptr::null_mut(), wide(class).as_ptr(), std::ptr::null()) }
}

fn rect_of(hwnd: HWND) -> Option<RECT> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    (!hwnd.is_null() && unsafe { GetWindowRect(hwnd, &mut rect) } != 0).then_some(rect)
}

/// Runs every second and on "TaskbarCreated": Explorer destroys our child when
/// it restarts, and the tray widens as icons come and go.
fn reconcile(s: &mut State) {
    unsafe {
        let tray = find(std::ptr::null_mut(), "Shell_TrayWnd");
        if s.tray != tray || (!s.window.is_null() && IsWindow(s.window) == 0) {
            if !s.window.is_null() && IsWindow(s.window) != 0 {
                DestroyWindow(s.window);
            }
            s.window = std::ptr::null_mut();
            s.tray = tray;
            s.embedded = false;
            s.bounds = (0, 0, 0, 0);
        }
        let Some(tray_rect) = rect_of(tray) else { return };
        if !s.visible {
            if !s.window.is_null() {
                ShowWindow(s.window, SW_HIDE);
            }
            return;
        }
        if s.window.is_null() {
            let class = wide(STATUS_CLASS);
            s.window = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            if s.window.is_null() {
                return;
            }
        }
        if !s.embedded {
            s.embedded = embed(s.window, tray);
            s.bounds = (0, 0, 0, 0);
        }
        let dpi = GetDpiForWindow(tray).max(96) as i32;
        let (width, height) = (WIDTH_AT_96_DPI * dpi / 96, HEIGHT_AT_96_DPI * dpi / 96);
        let tray_height = tray_rect.bottom - tray_rect.top;
        let notify_left = rect_of(find(tray, "TrayNotifyWnd")).map_or(0, |r| r.left - tray_rect.left);
        let mut x = place_x(notify_left, tray_rect.right - tray_rect.left, width);
        let mut y = (tray_height - height) / 2;
        if !s.embedded {
            x += tray_rect.left;
            y += tray_rect.top;
        }
        let bounds = (x, y, width, height);
        if bounds != s.bounds {
            s.bounds = bounds;
            let after = if s.embedded { HWND_TOP } else { HWND_TOPMOST };
            SetWindowPos(s.window, after, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            paint(s);
        } else if IsWindowVisible(s.window) == 0 {
            ShowWindow(s.window, SW_SHOWNOACTIVATE);
        }
    }
}

fn embed(window: HWND, tray: HWND) -> bool {
    unsafe {
        SetWindowLongPtrW(window, GWL_STYLE, (WS_CHILD | WS_CLIPSIBLINGS) as isize);
        if SetParent(window, tray).is_null() {
            SetWindowLongPtrW(window, GWL_STYLE, WS_POPUP as isize);
            return false;
        }
        true
    }
}

fn paint(s: &mut State) {
    let (_, _, width, height) = s.bounds;
    if s.window.is_null() || width <= 0 || height <= 0 {
        return;
    }
    let _frame = s.frame.lock().unwrap().take();
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if !bitmap.is_null() && !bits.is_null() {
            let old = SelectObject(dc, bitmap);
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
            // Placeholder: half-transparent blue (premultiplied).
            pixels.fill(0x80_00_40_80);
            let size = SIZE { cx: width, cy: height };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            if UpdateLayeredWindow(s.window, screen, std::ptr::null(), &size, dc, &source, 0, &blend, ULW_ALPHA) == 0 {
                eprintln!("taskbar status: UpdateLayeredWindow failed: {}", GetLastError());
            }
            SelectObject(dc, old);
            DeleteObject(bitmap);
        }
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_x_uses_notify_left_then_falls_back() {
        assert_eq!(place_x(1500, 1920, 95), 1405);
        assert_eq!(place_x(0, 1920, 95), 1825);
    }
}
