//! Renders one frame into premultiplied BGRA pixels for UpdateLayeredWindow.

use std::time::Duration;

use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, GetTextExtentPoint32W, SelectObject,
    SetBkMode, SetTextColor, TextOutW, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HDC, TRANSPARENT,
};
use windows_sys::Win32::Foundation::SIZE;

use super::view::{chart_points, format_rate, DelayView, Screen, Tone};
use crate::desktop::{CaptureMode, TaskbarTraffic};

const HEIGHT: i32 = 36;
const CHART_W: i32 = 26;
/// Fits the widest rate `format_rate` gives, "999M".
const VALUES_W: i32 = 34;
const NODE_W: i32 = 36;
const GAP: i32 = 6;
/// Breathing room from the last task button and from the tray arrow.
const PAD_LEFT: i32 = 2;
const PAD_RIGHT: i32 = 4;
const FONT_H: i32 = 12;
/// The capture icon stands in front of the delay, in place of its dot.
const ICON_W: i32 = 12;
const ICON_GAP: i32 = 2;
const ICON_FONT_H: i32 = 11;
/// Segoe Fluent Icons, shipped with Windows 11 (the only system the bar runs
/// on): a PC tower on a network cable for system proxy, a monitor for TUN.
const ICON_FONT: &str = "Segoe Fluent Icons";
const SYSTEM_ICON: &str = "\u{E968}";
const TUN_ICON: &str = "\u{E7F4}";
const SCROLL_PAUSE: Duration = Duration::from_secs(1);
const SCROLL_PX_PER_SEC: f32 = 24.0;
/// Keeps transparent pixels clickable: a zero alpha lets the mouse through.
const HIT_TEST_PIXEL: u32 = 0x0100_0000;
const AREA_COVERAGE: u32 = 255;
/// Chart colors taken from Proxifier's tray graph, on either taskbar theme.
const CHART_UP: u32 = 0x4676d5;
const CHART_DOWN: u32 = 0x2ec46e;
/// Chart margins at 96 DPI; the bottom one clears the running-app indicator.
const CHART_TOP: f32 = 4.0;
const CHART_BOTTOM: f32 = 6.0;
/// Text is drawn this many times larger and averaged down: GDI's own
/// antialiasing is mostly switched off by the font's hinting at small sizes.
const TEXT_SUPERSAMPLE: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub text: u32,
    pub dim: u32,
    pub up: u32,
    pub down: u32,
    pub good: u32,
    pub warn: u32,
    pub bad: u32,
}

/// Rate numbers use the chart's blue (up) and green (down), a shade darker on a
/// light taskbar where the bright ones wash out.
pub fn palette(light: bool) -> Palette {
    if light {
        Palette { text: 0x0f172a, dim: 0x64748b, up: 0x3563c9, down: 0x128a48, good: 0x059669, warn: 0xd97706, bad: 0xe11d48 }
    } else {
        Palette { text: 0xffffff, dim: 0xa5b4c3, up: 0x6b93ee, down: 0x2ec46e, good: 0x34e89e, warn: 0xffd43b, bad: 0xff6b81 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub traffic: TaskbarTraffic,
    pub pad_left: i32,
    pub pad_right: i32,
    /// The left column: the chart or the rate numbers.
    pub traffic_w: i32,
    pub width: i32,
    pub height: i32,
    pub chart_w: i32,
    pub values_w: i32,
    pub node_w: i32,
    pub gap: i32,
    pub font_h: i32,
    pub icon_w: i32,
    pub icon_gap: i32,
    pub icon_font_h: i32,
}

pub fn layout(dpi: u32, traffic: TaskbarTraffic) -> Layout {
    let scale = |v: i32| v * dpi as i32 / 96;
    let traffic_w = scale(match traffic {
        TaskbarTraffic::Values => VALUES_W,
        TaskbarTraffic::Chart => CHART_W,
    });
    Layout {
        traffic,
        traffic_w,
        pad_left: scale(PAD_LEFT),
        pad_right: scale(PAD_RIGHT),
        width: scale(PAD_LEFT) + traffic_w + scale(GAP) + scale(NODE_W) + scale(PAD_RIGHT),
        height: scale(HEIGHT),
        chart_w: scale(CHART_W),
        values_w: scale(VALUES_W),
        node_w: scale(NODE_W),
        gap: scale(GAP),
        font_h: scale(FONT_H),
        icon_w: scale(ICON_W),
        icon_gap: scale(ICON_GAP),
        icon_font_h: scale(ICON_FONT_H),
    }
}

/// Pause, scroll to the end, pause. The bool says the name was read in full.
pub fn scroll_offset(text_w: i32, column_w: i32, elapsed: Duration) -> (i32, bool) {
    let overflow = text_w - column_w;
    if overflow <= 0 {
        return (0, true);
    }
    let scrolling = elapsed.saturating_sub(SCROLL_PAUSE).as_secs_f32();
    let offset = ((scrolling * SCROLL_PX_PER_SEC) as i32).min(overflow);
    let duration = overflow as f32 / SCROLL_PX_PER_SEC + SCROLL_PAUSE.as_secs_f32();
    (offset, scrolling >= duration)
}

pub struct Frame {
    pub screen: Screen,
    /// The screen sliding out while `slide_progress` < 1.
    pub previous: Option<Screen>,
    pub slide_progress: f32,
    pub name_elapsed: Duration,
    pub light: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Painted {
    /// The current node name was read to its end (or fits).
    pub scroll_done: bool,
    /// Rendered width of the current node name; 0 without one.
    pub name_width: i32,
}

pub fn paint(frame: &Frame, layout: &Layout, pixels: &mut [u32]) -> Painted {
    pixels.fill(HIT_TEST_PIXEL);
    let Some(mut painter) = Painter::new(*layout, palette(frame.light), pixels) else {
        return Painted { scroll_done: true, name_width: 0 };
    };
    let progress = frame.slide_progress.clamp(0.0, 1.0);
    if let Some(previous) = frame.previous.as_ref().filter(|_| progress < 1.0) {
        painter.screen(previous, -((progress * layout.height as f32) as i32), Duration::ZERO);
    }
    let dy = ((1.0 - progress) * layout.height as f32) as i32;
    painter.screen(&frame.screen, dy, frame.name_elapsed)
}

/// Source-over of `rgb` at `coverage` onto a premultiplied pixel.
fn blend(pixel: &mut u32, rgb: u32, coverage: u32) {
    let keep = 255 - coverage;
    let mix = |shift: u32| {
        let source = ((rgb >> shift) & 255) * coverage / 255;
        (source + ((*pixel >> shift) & 255) * keep / 255) << shift
    };
    let alpha = coverage + (*pixel >> 24) * keep / 255;
    *pixel = (alpha << 24) | mix(16) | mix(8) | mix(0);
}

struct Painter<'a> {
    layout: Layout,
    palette: Palette,
    pixels: &'a mut [u32],
    dc: HDC,
    bitmap: *mut core::ffi::c_void,
    font: *mut core::ffi::c_void,
    icon_font: *mut core::ffi::c_void,
    mask: *mut u32,
}

/// Inked area of the mask in supersampled pixels: left, right, top, bottom (exclusive).
type Ink = (i32, i32, i32, i32);

impl<'a> Painter<'a> {
    fn new(layout: Layout, palette: Palette, pixels: &'a mut [u32]) -> Option<Self> {
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = layout.width * TEXT_SUPERSAMPLE;
            info.bmiHeader.biHeight = -layout.height * TEXT_SUPERSAMPLE;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            SelectObject(dc, bitmap);
            let face: Vec<u16> = "Microsoft YaHei UI".encode_utf16().chain(Some(0)).collect();
            let font = CreateFontW(
                -layout.font_h * TEXT_SUPERSAMPLE, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, ANTIALIASED_QUALITY as _, 0, face.as_ptr(),
            );
            let icon_face: Vec<u16> = ICON_FONT.encode_utf16().chain(Some(0)).collect();
            let icon_font = CreateFontW(
                -layout.icon_font_h * TEXT_SUPERSAMPLE, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, ANTIALIASED_QUALITY as _, 0,
                icon_face.as_ptr(),
            );
            SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as _);
            SetTextColor(dc, 0x00FF_FFFF);
            Some(Self { layout, palette, pixels, dc, bitmap, font, icon_font, mask: bits as *mut u32 })
        }
    }

    /// In output pixels.
    fn text_size(&self, text: &[u16]) -> SIZE {
        let mut size = SIZE { cx: 0, cy: 0 };
        unsafe { GetTextExtentPoint32W(self.dc, text.as_ptr(), text.len() as i32, &mut size) };
        SIZE { cx: size.cx / TEXT_SUPERSAMPLE, cy: size.cy / TEXT_SUPERSAMPLE }
    }

    fn mask(&mut self) -> &mut [u32] {
        let (w, h, n) = (self.layout.width, self.layout.height, TEXT_SUPERSAMPLE);
        unsafe { std::slice::from_raw_parts_mut(self.mask, (w * h * n * n) as usize) }
    }

    /// Clears the mask and draws `text` at supersampled `(x, y)`.
    fn draw(&mut self, font: *mut core::ffi::c_void, text: &[u16], x: i32, y: i32) {
        self.mask().fill(0);
        unsafe {
            SelectObject(self.dc, font);
            TextOutW(self.dc, x, y, text.as_ptr(), text.len() as i32);
            SelectObject(self.dc, self.font);
        }
    }

    fn ink(&mut self) -> Option<Ink> {
        let row = self.layout.width * TEXT_SUPERSAMPLE;
        let mut ink: Option<Ink> = None;
        for (i, m) in self.mask().iter().enumerate() {
            if (m >> 8) & 255 >= 128 {
                let (x, y) = (i as i32 % row, i as i32 / row);
                ink = Some(ink.map_or((x, x + 1, y, y + 1), |(l, r, t, b)| (l.min(x), r.max(x + 1), t.min(y), b.max(y + 1))));
            }
        }
        ink
    }

    /// GDI leaves alpha at zero, so text goes white-on-black into the mask and
    /// its brightness becomes the coverage of the wanted color.
    fn text(&mut self, text: &str, x: i32, center_y: i32, clip: (i32, i32), rgb: u32, offset: i32) {
        let n = TEXT_SUPERSAMPLE;
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let top = center_y - self.text_size(&utf16).cy / 2;
        self.draw(self.font, &utf16, (x - offset) * n, top * n);
        self.composite(clip, rgb);
    }

    /// Centers the icon in its column and on the ink of digits drawn on the
    /// same line: the icon font's cell sits lower than Microsoft YaHei's.
    fn icon(&mut self, icon: &str, x: i32, center_y: i32, rgb: u32) {
        let (h, n) = (self.layout.height, TEXT_SUPERSAMPLE);
        let digits: Vec<u16> = "0123456789".encode_utf16().collect();
        // Measured on the middle line, where a sliding screen cannot clip it.
        self.draw(self.font, &digits, 0, (h / 2 - self.text_size(&digits).cy / 2) * n);
        let Some((_, _, top, bottom)) = self.ink() else { return };
        let middle = (top + bottom) / 2 + (center_y - h / 2) * n;
        let glyph: Vec<u16> = icon.encode_utf16().collect();
        self.draw(self.icon_font, &glyph, 0, 0);
        let Some((left, right, top, bottom)) = self.ink() else { return };
        let icon_w = self.layout.icon_w;
        self.draw(self.icon_font, &glyph, x * n + (icon_w * n - (right - left)) / 2 - left, middle - (top + bottom) / 2);
        self.composite((x, x + icon_w), rgb);
    }

    fn composite(&mut self, clip: (i32, i32), rgb: u32) {
        let (w, h, n) = (self.layout.width, self.layout.height, TEXT_SUPERSAMPLE);
        let mask = unsafe { std::slice::from_raw_parts(self.mask, (w * h * n * n) as usize) };
        for py in 0..h {
            for px in clip.0.max(0)..clip.1.min(w) {
                let mut sum = 0;
                for sy in py * n..(py + 1) * n {
                    for sx in px * n..(px + 1) * n {
                        sum += (mask[(sy * w * n + sx) as usize] >> 8) & 255;
                    }
                }
                let coverage = sum / (n * n) as u32;
                if coverage > 0 {
                    blend(&mut self.pixels[(py * w + px) as usize], rgb, coverage);
                }
            }
        }
    }

    fn chart(&mut self, series: &[crate::stats::traffic::Rate], dy: i32) {
        // Nothing at all beats a flat line, which reads as a running indicator.
        if series.iter().all(|r| r.up == 0 && r.down == 0) {
            return;
        }
        let (w, h, chart_w) = (self.layout.width, self.layout.height, self.layout.chart_w);
        let scale = h as f32 / HEIGHT as f32;
        // The baseline sits above the strip where Windows draws running indicators.
        let (top, base) = (CHART_TOP * scale, h as f32 - CHART_BOTTOM * scale);
        // Stacked like Proxifier's tray graph: up at the bottom, down on top of it,
        // so neither hides the other and the outline is the total.
        let stacked: Vec<_> = series
            .iter()
            .map(|r| crate::stats::traffic::Rate { up: r.up, down: r.up + r.down })
            .collect();
        let (up, down) = chart_points(&stacked, (chart_w - 1) as f32, base - top);
        let shift = |line: Vec<(f32, f32)>| line.into_iter().map(|(x, y)| (x, y + top)).collect::<Vec<_>>();
        let (up, down) = (shift(up), shift(down));
        for py in 0..h {
            let local = (py - dy) as f32 + 0.5;
            if local < 0.0 || local > base {
                continue;
            }
            for cx in 0..chart_w {
                let px = cx + self.layout.pad_left;
                let point = (cx as f32 + 0.5, local);
                let index = (py * w + px) as usize;
                for (line, rgb) in [(&down, CHART_DOWN), (&up, CHART_UP)] {
                    let surface = line
                        .windows(2)
                        .find(|s| point.0 >= s[0].0 && point.0 <= s[1].0)
                        .map(|s| s[0].1 + (s[1].1 - s[0].1) * (point.0 - s[0].0) / (s[1].0 - s[0].0));
                    if let Some(y) = surface {
                        let edge = (local - y + 0.5).clamp(0.0, 1.0);
                        if edge > 0.0 {
                            blend(&mut self.pixels[index], rgb, (edge * AREA_COVERAGE as f32) as u32);
                        }
                    }
                }
            }
        }
    }

    fn screen(&mut self, screen: &Screen, dy: i32, name_elapsed: Duration) -> Painted {
        let Layout { traffic, width, height, values_w, node_w, .. } = self.layout;
        let (top, bottom) = (height / 4 + dy, height * 3 / 4 + dy);
        let node_x = width - self.layout.pad_right - node_w;
        let node_clip = (node_x, node_x + node_w);
        let left = self.layout.pad_left;
        let palette = self.palette;
        match screen {
            Screen::NotRunning | Screen::NoListeners => {
                let label = if matches!(screen, Screen::NotRunning) { "未运行" } else { "无监听" };
                self.text(label, node_x, height / 2 + dy, node_clip, palette.dim, 0);
                Painted { scroll_done: true, name_width: 0 }
            }
            Screen::Listener(view) => {
                match traffic {
                    TaskbarTraffic::Chart => self.chart(&view.series, dy),
                    TaskbarTraffic::Values => {
                        let clip = (left, left + values_w);
                        self.text(&format_rate(view.up), left, top, clip, palette.up, 0);
                        self.text(&format_rate(view.down), left, bottom, clip, palette.down, 0);
                    }
                }
                let name: Vec<u16> = view.node.encode_utf16().collect();
                let name_width = self.text_size(&name).cx;
                let (offset, scroll_done) = scroll_offset(name_width, node_w, name_elapsed);
                self.text(&view.node, node_x, top, node_clip, palette.text, offset);
                let (ms, rgb) = match view.delay {
                    DelayView::Ms(ms, tone) => (
                        ms.to_string(),
                        match tone {
                            Tone::Good => palette.good,
                            Tone::Warn => palette.warn,
                            Tone::Bad => palette.bad,
                        },
                    ),
                    DelayView::Failed => ("--".to_string(), palette.bad),
                    DelayView::Unknown => ("--".to_string(), palette.dim),
                };
                // The icon tells system proxy from TUN; its color still grades the delay.
                match view.capture {
                    Some(mode) => {
                        let icon = if mode == CaptureMode::Tun { TUN_ICON } else { SYSTEM_ICON };
                        self.icon(icon, node_x, bottom, rgb);
                        let text_x = node_x + self.layout.icon_w + self.layout.icon_gap;
                        self.text(&ms, text_x, bottom, node_clip, rgb, 0);
                    }
                    None => self.text(&format!("● {ms}"), node_x, bottom, node_clip, rgb, 0),
                }
                Painted { scroll_done, name_width }
            }
        }
    }
}

impl Drop for Painter<'_> {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.dc);
            DeleteObject(self.bitmap);
            DeleteObject(self.font);
            DeleteObject(self.icon_font);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::TaskbarTraffic::{self, Chart, Values};
    use crate::model::ListenerId;
    use crate::stats::traffic::Rate;
    use crate::taskbar_status::view::{DelayView, ListenerView, Tone};

    fn listener_screen() -> Screen {
        Screen::Listener(ListenerView {
            id: ListenerId("l".into()),
            node: "香港 IEPL 01 高速专线 🚀".into(),
            delay: DelayView::Ms(42, Tone::Good),
            up: 12_400,
            down: 3_355_443,
            series: (0..10).map(|i| Rate { up: i, down: i * 10 }).collect(),
            capture: None,
        })
    }

    fn frame(screen: Screen) -> Frame {
        Frame { screen, previous: None, slide_progress: 1.0, name_elapsed: Duration::ZERO, light: false }
    }

    /// Whether `ICON_FONT` is installed. It ships with Windows 11 only; the CI
    /// runners (Windows Server 2022) lack it, so icon glyphs draw nothing there.
    fn icon_font_installed() -> bool {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        std::path::Path::new(&windir).join("Fonts").join("SegoeIcons.ttf").is_file()
    }

    fn painted(frame: &Frame) -> (Vec<u32>, Painted) {
        painted_in(frame, Values)
    }

    fn painted_in(frame: &Frame, traffic: TaskbarTraffic) -> (Vec<u32>, Painted) {
        let layout = layout(96, traffic);
        let mut pixels = vec![0; (layout.width * layout.height) as usize];
        let done = paint(frame, &layout, &mut pixels);
        (pixels, done)
    }

    #[test]
    fn layout_keeps_only_the_chosen_traffic_view() {
        assert_eq!((layout(96, Values).width, layout(96, Values).traffic_w), (82, 34));
        assert_eq!((layout(96, Chart).width, layout(96, Chart).traffic_w), (74, 26));
        for traffic in [Values, Chart] {
            let (one, two) = (layout(96, traffic), layout(192, traffic));
            assert_eq!(one.node_w, 36);
            assert_eq!((two.width, two.height, two.node_w), (one.width * 2, one.height * 2, 72));
        }
    }

    #[test]
    fn scroll_offset_static_when_text_fits() {
        assert_eq!(scroll_offset(30, 36, Duration::from_secs(99)), (0, true));
    }

    #[test]
    fn scroll_offset_pauses_then_scrolls_then_done() {
        assert_eq!(scroll_offset(100, 36, Duration::ZERO), (0, false));
        assert_eq!(scroll_offset(100, 36, Duration::from_millis(900)), (0, false));
        let mut last = 0;
        for ms in (1000..4200).step_by(100) {
            let (offset, done) = scroll_offset(100, 36, Duration::from_millis(ms));
            assert!(offset >= last && offset <= 64);
            assert!(!done);
            last = offset;
        }
        assert_eq!(last, 64);
        assert_eq!(scroll_offset(100, 36, Duration::from_secs(10)), (64, true));
    }

    #[test]
    fn paint_keeps_every_pixel_hit_testable() {
        for screen in [Screen::NotRunning, Screen::NoListeners, listener_screen()] {
            let (pixels, _) = painted(&frame(screen));
            assert!(pixels.iter().all(|p| p >> 24 >= 1));
        }
    }

    #[test]
    fn paint_draws_traffic_and_node_columns() {
        for traffic in [Values, Chart] {
            let (pixels, _) = painted_in(&frame(listener_screen()), traffic);
            let layout = layout(96, traffic);
            let column = |range: std::ops::Range<i32>| {
                pixels
                    .chunks(layout.width as usize)
                    .any(|row| range.clone().any(|x| row[x as usize] >> 24 > 1))
            };
            assert!(column(0..layout.traffic_w));
            assert!(column(layout.width - layout.node_w..layout.width));
        }
    }

    fn with_traffic(series: Vec<Rate>, up: u64, down: u64) -> Screen {
        let Screen::Listener(mut view) = listener_screen() else { unreachable!() };
        (view.series, view.up, view.down) = (series, up, down);
        Screen::Listener(view)
    }

    #[test]
    fn values_view_ignores_the_chart_series() {
        let a = painted_in(&frame(with_traffic(vec![Rate::default(); 10], 5, 5)), Values).0;
        let b = painted_in(&frame(with_traffic(vec![Rate { up: 9, down: 9 }; 10], 5, 5)), Values).0;
        assert_eq!(a, b);
    }

    #[test]
    fn chart_view_ignores_the_rate_numbers() {
        let series = (0..10).map(|i| Rate { up: i, down: i * 10 }).collect::<Vec<_>>();
        let a = painted_in(&frame(with_traffic(series.clone(), 1, 1)), Chart).0;
        let b = painted_in(&frame(with_traffic(series, 3_000_000, 3_000_000)), Chart).0;
        assert_eq!(a, b);
    }

    #[test]
    fn paint_output_is_premultiplied() {
        let (pixels, _) = painted(&frame(listener_screen()));
        for p in pixels {
            let (a, r, g, b) = (p >> 24, (p >> 16) & 255, (p >> 8) & 255, p & 255);
            assert!(r <= a && g <= a && b <= a, "{p:#x}");
        }
    }

    #[test]
    fn paint_reports_scroll_done_only_after_a_long_name_finished() {
        let mut f = frame(listener_screen());
        assert!(!painted(&f).1.scroll_done);
        f.name_elapsed = Duration::from_secs(120);
        assert!(painted(&f).1.scroll_done);
        assert!(painted(&frame(Screen::NotRunning)).1.scroll_done);
    }

    /// Share of inked pixels that are edge shades rather than solid or empty.
    fn edge_share(screen: Screen) -> f32 {
        let (pixels, _) = painted(&frame(screen));
        let alphas: Vec<u32> = pixels.iter().map(|p| p >> 24).filter(|a| *a > 1).collect();
        alphas.iter().filter(|a| **a < 230).count() as f32 / alphas.len() as f32
    }

    #[test]
    fn text_edges_are_smoothly_antialiased() {
        // Hinted GDI text at this size is mostly solid pixels with jagged edges.
        let share = edge_share(Screen::NotRunning);
        assert!(share > 0.5, "edge share {share}");
    }

    fn listener_with(series: Vec<Rate>) -> Screen {
        let Screen::Listener(mut view) = listener_screen() else { unreachable!() };
        view.series = series;
        Screen::Listener(view)
    }

    /// Whether anything but the transparent hit-test pixel lies in these chart rows.
    fn chart_inked(screen: Screen, rows: std::ops::Range<i32>) -> bool {
        let (pixels, _) = painted_in(&frame(screen), Chart);
        let layout = layout(96, Chart);
        rows.flat_map(|y| (0..layout.traffic_w).map(move |x| (y * layout.width + x) as usize))
            .any(|i| pixels[i] >> 24 > 1)
    }

    #[test]
    fn chart_is_empty_without_traffic() {
        // A flat line on the bottom edge reads as another app's running indicator.
        assert!(!chart_inked(listener_with(vec![Rate::default(); 10]), 0..layout(96, Chart).height));
    }

    #[test]
    fn chart_stays_clear_of_the_taskbar_running_indicator() {
        let h = layout(96, Chart).height;
        let series = (0..10).map(|i| Rate { up: 0, down: i % 2 * 1000 }).collect();
        assert!(chart_inked(listener_with(series), 0..h));
        let series = (0..10).map(|i| Rate { up: 0, down: i % 2 * 1000 }).collect();
        assert!(!chart_inked(listener_with(series), h - 4..h));
    }

    #[test]
    fn paint_reports_the_node_name_width() {
        assert!(painted(&frame(listener_screen())).1.name_width > layout(96, Values).node_w);
        assert_eq!(painted(&frame(Screen::NotRunning)).1.name_width, 0);
    }

    #[test]
    fn a_slide_mid_way_shows_both_screens_without_panicking() {
        let mut f = frame(listener_screen());
        f.previous = Some(Screen::NotRunning);
        f.slide_progress = 0.5;
        assert!(painted(&f).0.iter().all(|p| p >> 24 >= 1));
    }

    #[test]
    fn palette_changes_with_the_theme() {
        assert_ne!(palette(true).text, palette(false).text);
        assert_eq!((palette(false).up, palette(false).down), (0x6b93ee, 0x2ec46e));
        assert_eq!((palette(true).up, palette(true).down), (0x3563c9, 0x128a48));
    }

    #[test]
    fn bar_keeps_clear_margins_to_its_neighbours() {
        for traffic in [Values, Chart] {
            let (pixels, _) = painted_in(&frame(listener_screen()), traffic);
            let l = layout(96, traffic);
            let margin = (0..l.pad_left).chain(l.width - l.pad_right..l.width);
            let inked = margin.flat_map(|x| (0..l.height).map(move |y| (y * l.width + x) as usize)).any(|i| pixels[i] >> 24 > 1);
            assert!(!inked, "{traffic:?}");
        }
    }

    #[test]
    fn stacked_chart_puts_up_below_down_in_proxifier_colors() {
        let (pixels, _) = painted_in(&frame(with_traffic(vec![Rate { up: 1000, down: 1000 }; 10], 0, 0)), Chart);
        let l = layout(96, Chart);
        let x = l.pad_left + l.traffic_w / 2;
        let rows = |rgb: u32| (0..l.height).filter(|y| pixels[(y * l.width + x) as usize] == 0xff00_0000 | rgb).collect::<Vec<_>>();
        let (up, down) = (rows(CHART_UP), rows(CHART_DOWN));
        assert!(!up.is_empty() && !down.is_empty());
        assert!(up.iter().min() > down.iter().max());
    }

    #[test]
    fn every_rate_fits_the_values_column() {
        let widest = ["999B", "999K", "999M", "999G", "9.9K", "8.8M", "1.0G", "888M", "100M"];
        for dpi in [96, 120, 144, 168, 192] {
            let l = layout(dpi, Values);
            let mut pixels = vec![0; (l.width * l.height) as usize];
            let painter = Painter::new(l, palette(false), &mut pixels).unwrap();
            for text in widest {
                let width = painter.text_size(&text.encode_utf16().collect::<Vec<_>>()).cx;
                assert!(width <= l.values_w, "{text} is {width}px at {dpi} DPI, column {}", l.values_w);
            }
        }
    }

    fn captured(mode: Option<CaptureMode>) -> Screen {
        let Screen::Listener(mut view) = listener_screen() else { unreachable!() };
        view.capture = mode;
        Screen::Listener(view)
    }

    /// Inked rows (top, bottom exclusive) in the delay line between these columns.
    fn delay_ink(pixels: &[u32], l: &Layout, columns: std::ops::Range<i32>) -> Option<(i32, i32)> {
        let rows: Vec<i32> = (l.height / 2..l.height)
            .filter(|y| columns.clone().any(|x| pixels[(y * l.width + x) as usize] >> 24 > 96))
            .collect();
        Some((*rows.first()?, *rows.last()? + 1))
    }

    #[test]
    fn capture_icon_sits_level_with_the_delay() {
        if !icon_font_installed() {
            eprintln!("skipped: {ICON_FONT} is not installed");
            return;
        }
        for dpi in [96, 144] {
            let l = layout(dpi, Values);
            for mode in [CaptureMode::System, CaptureMode::Tun] {
                let mut pixels = vec![0; (l.width * l.height) as usize];
                paint(&frame(captured(Some(mode))), &l, &mut pixels);
                let node_x = l.width - l.pad_right - l.node_w;
                let icon = delay_ink(&pixels, &l, node_x..node_x + l.icon_w).expect("icon drawn");
                let digits_x = node_x + l.icon_w + l.icon_gap;
                let digits = delay_ink(&pixels, &l, digits_x..node_x + l.node_w).expect("delay drawn");
                let (icon_mid, digits_mid) = (icon.0 + icon.1, digits.0 + digits.1);
                assert!((icon_mid - digits_mid).abs() <= 2, "{mode:?} at {dpi}: icon {icon:?}, digits {digits:?}");
            }
        }
    }

    #[test]
    fn system_proxy_and_tun_have_different_icons() {
        if !icon_font_installed() {
            eprintln!("skipped: {ICON_FONT} is not installed");
            return;
        }
        let system = painted(&frame(captured(Some(CaptureMode::System)))).0;
        let tun = painted(&frame(captured(Some(CaptureMode::Tun)))).0;
        let plain = painted(&frame(captured(None))).0;
        assert_ne!(system, tun);
        assert_ne!(system, plain);
        assert_ne!(tun, plain);
    }
}
