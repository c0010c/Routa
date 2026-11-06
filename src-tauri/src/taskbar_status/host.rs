//! The status window: a layered child of the Windows 11 taskbar, driven by one
//! dedicated thread. Nothing here touches the runtime, so this thread never
//! waits on async work while Explorer's input queue is attached to it.

use std::cell::RefCell;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::paint::{layout, paint, scroll_offset, Frame, Layout, Painted};
use super::view::{ListenerView, Rotation, Screen};
use crate::desktop::{TaskbarStatusMode, TaskbarTraffic};
use crate::model::ListenerId;

const WM_SNAPSHOT: u32 = WM_APP + 1;
const WM_VISIBLE: u32 = WM_APP + 2;
const RECONCILE_TIMER: usize = 1;
const ANIMATION_TIMER: usize = 2;
const ANIMATION_MS: u32 = 50;
const SLIDE: Duration = Duration::from_millis(250);
const CLOSE_COMMAND: usize = 1;
const CONTROL_CLASS: &str = "RoutaTaskbarStatusControl";
const STATUS_CLASS: &str = "RoutaTaskbarStatus";
/// The auto-hidden taskbar leaves about this much on screen.
const MIN_VISIBLE_TASKBAR_DIVISOR: i32 = 2;

/// One second of data; the window thread decides which listener is on screen.
pub struct Snapshot {
    pub running: bool,
    /// The rotation candidates, in display order.
    pub views: Vec<ListenerView>,
    pub mode: TaskbarStatusMode,
    pub traffic: TaskbarTraffic,
}

impl Snapshot {
    fn ids(&self) -> Vec<ListenerId> {
        self.views.iter().map(|v| v.id.clone()).collect()
    }

    fn screen(&self, current: Option<&ListenerId>) -> Screen {
        if !self.running {
            return Screen::NotRunning;
        }
        current
            .and_then(|id| self.views.iter().find(|v| &v.id == id))
            .map_or(Screen::NoListeners, |v| Screen::Listener(v.clone()))
    }
}

/// Taskbar-client x of the window: left of the notification area, or the right
/// edge of the taskbar when that area cannot be measured.
pub fn place_x(notify_left: i32, tray_width: i32, width: i32) -> i32 {
    if notify_left > 0 {
        notify_left - width
    } else {
        tray_width - width
    }
}

/// Whether the topmost fallback window may show. Embedded, the bar simply goes
/// wherever the taskbar goes; on top of it, it has to step aside by itself.
pub fn fallback_visible(fullscreen: bool, tray_visible_px: i32, tray_h: i32) -> bool {
    !fullscreen && tray_visible_px >= tray_h / MIN_VISIBLE_TASKBAR_DIVISOR
}

/// What is on screen, as far as deciding whether to paint again goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub screen: Screen,
    pub offset: i32,
    pub light: bool,
    pub bounds: (i32, i32, i32, i32),
}

/// A long name pauses before and after each scroll; during those pauses the
/// animation timer keeps ticking but the pixels would come out the same.
pub fn skip_redraw(last: Option<&Drawn>, next: &Drawn, sliding: bool) -> bool {
    !sliding && last == Some(next)
}

/// A taskbar that refused our child once keeps refusing (security software),
/// so retrying every second would only flicker the fallback window.
pub fn should_try_embed(failed_for: isize, tray: isize) -> bool {
    failed_for != tray
}

/// The taskbar is topmost too and rises above the fallback on every click.
pub fn reclaim_topmost(embedded: bool, shown: bool) -> bool {
    !embedded && shown
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
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Host {
    /// `None` below Windows 11 or when the window thread cannot start.
    pub fn spawn(on_close: Box<dyn Fn() + Send>) -> Option<Host> {
        if !supported() {
            return None;
        }
        let snapshot = Arc::new(Mutex::new(None));
        let shared = snapshot.clone();
        let (ready, control) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("taskbar-status".into())
            .spawn(move || run(on_close, shared, ready))
            .ok()?;
        let control = control.recv().ok()??;
        Some(Host { control, snapshot, thread: Mutex::new(Some(thread)) })
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe { PostMessageW(self.control as HWND, WM_VISIBLE, visible as usize, 0) };
    }

    pub fn update(&self, snapshot: Snapshot) {
        *self.snapshot.lock().unwrap() = Some(snapshot);
        unsafe { PostMessageW(self.control as HWND, WM_SNAPSHOT, 0, 0) };
    }

    /// Destroys the window; the bar must not outlive the app on the taskbar.
    pub fn shutdown(&self) {
        let Some(thread) = self.thread.lock().unwrap().take() else { return };
        unsafe { PostMessageW(self.control as HWND, WM_CLOSE, 0, 0) };
        let deadline = Instant::now() + Duration::from_secs(1);
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

struct State {
    control: HWND,
    window: HWND,
    tray: HWND,
    embedded: bool,
    embed_failed_for: HWND,
    visible: bool,
    bounds: (i32, i32, i32, i32),
    layout: Layout,
    traffic: TaskbarTraffic,
    light: bool,
    taskbar_created: u32,
    on_close: Box<dyn Fn() + Send>,
    incoming: Arc<Mutex<Option<Snapshot>>>,
    snapshot: Option<Snapshot>,
    rotation: Rotation,
    current: Option<ListenerId>,
    previous: Option<Screen>,
    slide_started: Option<Instant>,
    name_started: Instant,
    scroll_done: bool,
    name_width: i32,
    last_drawn: Option<Drawn>,
    animating: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.try_borrow_mut().ok()?.as_mut().map(f))
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

fn run(
    on_close: Box<dyn Fn() + Send>,
    incoming: Arc<Mutex<Option<Snapshot>>>,
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
                embed_failed_for: std::ptr::null_mut(),
                visible: true,
                bounds: (0, 0, 0, 0),
                layout: layout(96, TaskbarTraffic::Values),
                traffic: TaskbarTraffic::Values,
                light: system_uses_light_theme(),
                taskbar_created,
                on_close,
                incoming,
                snapshot: None,
                rotation: Rotation::default(),
                current: None,
                previous: None,
                slide_started: None,
                name_started: Instant::now(),
                scroll_done: true,
                name_width: 0,
                last_drawn: None,
                animating: false,
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
        WM_TIMER if wparam == RECONCILE_TIMER => {
            with_state(reconcile);
            0
        }
        WM_TIMER if wparam == ANIMATION_TIMER => {
            with_state(render);
            0
        }
        WM_SNAPSHOT => {
            with_state(|s| {
                let incoming = s.incoming.lock().unwrap().take();
                if let Some(snapshot) = incoming {
                    let resized = snapshot.traffic != s.traffic;
                    s.traffic = snapshot.traffic;
                    s.snapshot = Some(snapshot);
                    if resized {
                        // A narrower or wider bar: reposition, which also repaints.
                        return reconcile(s);
                    }
                }
                render(s);
            });
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
            with_state(|s| {
                s.embed_failed_for = std::ptr::null_mut();
                reconcile(s)
            });
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn status_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Asking the taskbar (the parent) could eat the click.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
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

fn system_uses_light_theme() -> bool {
    let (key, value) = (wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"), wide("SystemUsesLightTheme"));
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    status == 0 && data != 0
}

fn fullscreen_app_running() -> bool {
    let mut state = 0;
    let queried = unsafe { SHQueryUserNotificationState(&mut state) } == 0;
    queried && matches!(state, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
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
            s.last_drawn = None;
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
        if !s.embedded && should_try_embed(s.embed_failed_for as isize, tray as isize) {
            if embed(s.window, tray) {
                s.embedded = true;
                s.bounds = (0, 0, 0, 0);
            } else {
                s.embed_failed_for = tray;
            }
        }
        s.layout = layout(GetDpiForWindow(tray).max(96), s.traffic);
        let tray_height = tray_rect.bottom - tray_rect.top;
        let notify_left = rect_of(find(tray, "TrayNotifyWnd")).map_or(0, |r| r.left - tray_rect.left);
        let mut x = place_x(notify_left, tray_rect.right - tray_rect.left, s.layout.width);
        let mut y = (tray_height - s.layout.height) / 2;
        let mut show = true;
        if !s.embedded {
            x += tray_rect.left;
            y += tray_rect.top;
            let on_screen = tray_rect.bottom.min(GetSystemMetrics(SM_CYSCREEN)) - tray_rect.top.max(0);
            show = fallback_visible(fullscreen_app_running(), on_screen, tray_height);
        }
        let bounds = (x, y, s.layout.width, s.layout.height);
        if !show {
            ShowWindow(s.window, SW_HIDE);
        } else if bounds != s.bounds {
            s.bounds = bounds;
            let after = if s.embedded { HWND_TOP } else { HWND_TOPMOST };
            SetWindowPos(s.window, after, x, y, bounds.2, bounds.3, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            render(s);
        } else if IsWindowVisible(s.window) == 0 {
            ShowWindow(s.window, SW_SHOWNOACTIVATE);
        }
        if reclaim_topmost(s.embedded, show) {
            SetWindowPos(s.window, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        let light = system_uses_light_theme();
        if light != s.light {
            s.light = light;
            render(s);
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

/// Advances rotation and animation, then draws. Idle, this runs once a second;
/// the fast timer only lives while something moves.
fn render(s: &mut State) {
    if s.window.is_null() || s.bounds.2 <= 0 || !s.visible {
        set_animating(s, false);
        return;
    }
    let now = Instant::now();
    let mut screen = Screen::NotRunning;
    if let Some(snapshot) = &s.snapshot {
        let next = s.rotation.next_slide(now, s.scroll_done, &snapshot.ids(), &snapshot.mode);
        if next != s.current {
            s.previous = s.current.as_ref().map(|id| snapshot.screen(Some(id)));
            s.slide_started = s.current.as_ref().map(|_| now);
            s.name_started = now;
            s.scroll_done = false;
            s.current = next;
        } else if s.scroll_done {
            // Still on screen after reading to the end: scroll it again.
            s.name_started = now;
        }
        screen = snapshot.screen(s.current.as_ref());
    }
    let progress = s.slide_started.map_or(1.0, |at| now.duration_since(at).as_secs_f32() / SLIDE.as_secs_f32());
    if progress >= 1.0 {
        s.slide_started = None;
        s.previous = None;
    }
    let name_elapsed = now.duration_since(s.name_started);
    let (offset, scroll_done) = scroll_offset(s.name_width, s.layout.node_w, name_elapsed);
    let drawn = Drawn { screen: screen.clone(), offset, light: s.light, bounds: s.bounds };
    let sliding = s.slide_started.is_some();
    if skip_redraw(s.last_drawn.as_ref(), &drawn, sliding) {
        s.scroll_done = scroll_done;
    } else {
        let frame = Frame { screen, previous: s.previous.clone(), slide_progress: progress, name_elapsed, light: s.light };
        let painted = draw(s, &frame);
        s.scroll_done = painted.scroll_done;
        s.name_width = painted.name_width;
        s.last_drawn = (!sliding).then_some(drawn);
    }
    set_animating(s, s.slide_started.is_some() || !s.scroll_done);
}

fn set_animating(s: &mut State, animating: bool) {
    if animating != s.animating {
        s.animating = animating;
        unsafe {
            if animating {
                SetTimer(s.control, ANIMATION_TIMER, ANIMATION_MS, None);
            } else {
                KillTimer(s.control, ANIMATION_TIMER);
            }
        }
    }
}

fn draw(s: &State, frame: &Frame) -> Painted {
    let Layout { width, height, .. } = s.layout;
    let mut painted = Painted { scroll_done: true, name_width: 0 };
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
            painted = paint(frame, &s.layout, pixels);
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
    painted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scroll_pause_that_shows_the_same_pixels_is_not_redrawn() {
        let drawn = Drawn { screen: Screen::NotRunning, offset: 4, light: false, bounds: (1, 2, 95, 36) };
        assert!(skip_redraw(Some(&drawn), &drawn.clone(), false));
        assert!(!skip_redraw(Some(&drawn), &drawn.clone(), true));
        assert!(!skip_redraw(None, &drawn, false));
        for changed in [
            Drawn { offset: 5, ..drawn.clone() },
            Drawn { light: true, ..drawn.clone() },
            Drawn { bounds: (0, 2, 95, 36), ..drawn.clone() },
            Drawn { screen: Screen::NoListeners, ..drawn.clone() },
        ] {
            assert!(!skip_redraw(Some(&drawn), &changed, false));
        }
    }

    #[test]
    fn embedding_is_retried_only_for_a_new_taskbar() {
        assert!(should_try_embed(0, 7));
        assert!(!should_try_embed(7, 7));
        assert!(should_try_embed(7, 8));
    }

    #[test]
    fn only_the_shown_fallback_window_reclaims_topmost() {
        assert!(reclaim_topmost(false, true));
        assert!(!reclaim_topmost(true, true));
        assert!(!reclaim_topmost(false, false));
    }

    #[test]
    fn fallback_hidden_for_fullscreen_or_autohide() {
        assert!(fallback_visible(false, 48, 48));
        assert!(!fallback_visible(true, 48, 48));
        assert!(!fallback_visible(false, 2, 48));
    }

    #[test]
    fn snapshot_screen_follows_state_and_current_listener() {
        use crate::taskbar_status::view::DelayView;
        let view = ListenerView {
            id: ListenerId("a".into()),
            node: "甲".into(),
            delay: DelayView::Unknown,
            up: 0,
            down: 0,
            series: vec![],
            capture: None,
        };
        let mut snapshot = Snapshot {
            running: true,
            views: vec![view.clone()],
            mode: TaskbarStatusMode::Rotate,
            traffic: TaskbarTraffic::Values,
        };
        assert_eq!(snapshot.screen(Some(&ListenerId("a".into()))), Screen::Listener(view));
        assert_eq!(snapshot.screen(None), Screen::NoListeners);
        snapshot.running = false;
        assert_eq!(snapshot.screen(Some(&ListenerId("a".into()))), Screen::NotRunning);
    }

    #[test]
    fn place_x_uses_notify_left_then_falls_back() {
        assert_eq!(place_x(1500, 1920, 95), 1405);
        assert_eq!(place_x(0, 1920, 95), 1825);
    }
}
