//! Renders one frame into premultiplied BGRA pixels for UpdateLayeredWindow.

use std::time::Duration;

use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, GetTextExtentPoint32W, SelectObject,
    SetBkMode, SetTextColor, TextOutW, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HDC, TRANSPARENT,
};
use windows_sys::Win32::Foundation::SIZE;

use super::view::{chart_points, format_rate, DelayView, Screen, Tone};

const WIDTH: i32 = 95;
const HEIGHT: i32 = 36;
const CHART_W: i32 = 26;
const VALUES_W: i32 = 27;
const NODE_W: i32 = 36;
const GAP: i32 = 3;
const FONT_H: i32 = 12;
const SCROLL_PAUSE: Duration = Duration::from_secs(1);
const SCROLL_PX_PER_SEC: f32 = 24.0;
/// Keeps transparent pixels clickable: a zero alpha lets the mouse through.
const HIT_TEST_PIXEL: u32 = 0x0100_0000;
const AREA_COVERAGE: u32 = 77;
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

/// Darker series colors on a light taskbar, where the bright ones wash out.
pub fn palette(light: bool) -> Palette {
    if light {
        Palette { text: 0x1f2937, dim: 0x6b7280, up: 0xb45309, down: 0x0369a1, good: 0x15803d, warn: 0xa16207, bad: 0xb91c1c }
    } else {
        Palette { text: 0xf2f5f8, dim: 0x9fb0bf, up: 0xfbbf24, down: 0x38bdf8, good: 0x4ade80, warn: 0xfacc15, bad: 0xf87171 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub chart_w: i32,
    pub values_w: i32,
    pub node_w: i32,
    pub gap: i32,
    pub font_h: i32,
}

pub fn layout(dpi: u32) -> Layout {
    let scale = |v: i32| v * dpi as i32 / 96;
    Layout {
        width: scale(WIDTH),
        height: scale(HEIGHT),
        chart_w: scale(CHART_W),
        values_w: scale(VALUES_W),
        node_w: scale(NODE_W),
        gap: scale(GAP),
        font_h: scale(FONT_H),
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

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

struct Painter<'a> {
    layout: Layout,
    palette: Palette,
    pixels: &'a mut [u32],
    dc: HDC,
    bitmap: *mut core::ffi::c_void,
    font: *mut core::ffi::c_void,
    mask: *mut u32,
}

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
                -layout.font_h * TEXT_SUPERSAMPLE, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, ANTIALIASED_QUALITY as _, 0, face.as_ptr(),
            );
            SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as _);
            SetTextColor(dc, 0x00FF_FFFF);
            Some(Self { layout, palette, pixels, dc, bitmap, font, mask: bits as *mut u32 })
        }
    }

    /// In output pixels.
    fn text_size(&self, text: &[u16]) -> SIZE {
        let mut size = SIZE { cx: 0, cy: 0 };
        unsafe { GetTextExtentPoint32W(self.dc, text.as_ptr(), text.len() as i32, &mut size) };
        SIZE { cx: size.cx / TEXT_SUPERSAMPLE, cy: size.cy / TEXT_SUPERSAMPLE }
    }

    /// GDI leaves alpha at zero, so text goes white-on-black into the mask and
    /// its brightness becomes the coverage of the wanted color.
    fn text(&mut self, text: &str, x: i32, center_y: i32, clip: (i32, i32), rgb: u32, offset: i32) {
        let (w, h, n) = (self.layout.width, self.layout.height, TEXT_SUPERSAMPLE);
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let top = center_y - self.text_size(&utf16).cy / 2;
        let mask = unsafe { std::slice::from_raw_parts_mut(self.mask, (w * h * n * n) as usize) };
        mask.fill(0);
        unsafe { TextOutW(self.dc, (x - offset) * n, top * n, utf16.as_ptr(), utf16.len() as i32) };
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
        let thickness = scale.max(1.0);
        // The baseline sits above the strip where Windows draws running indicators.
        let (top, base) = (CHART_TOP * scale, h as f32 - CHART_BOTTOM * scale);
        let (up, down) = chart_points(series, (chart_w - 1) as f32, base - top);
        let shift = |line: Vec<(f32, f32)>| line.into_iter().map(|(x, y)| (x, y + top)).collect::<Vec<_>>();
        let (up, down) = (shift(up), shift(down));
        for py in 0..h {
            let local = (py - dy) as f32 + 0.5;
            if local < 0.0 || local > base {
                continue;
            }
            for px in 0..chart_w {
                let point = (px as f32 + 0.5, local);
                let index = (py * w + px) as usize;
                let surface = down
                    .windows(2)
                    .find(|s| point.0 >= s[0].0 && point.0 <= s[1].0)
                    .map(|s| s[0].1 + (s[1].1 - s[0].1) * (point.0 - s[0].0) / (s[1].0 - s[0].0));
                if surface.is_some_and(|y| local > y) {
                    blend(&mut self.pixels[index], self.palette.down, AREA_COVERAGE);
                }
                for (line, rgb) in [(&down, self.palette.down), (&up, self.palette.up)] {
                    let distance = line
                        .windows(2)
                        .map(|s| segment_distance(point, s[0], s[1]))
                        .fold(f32::MAX, f32::min);
                    let coverage = (thickness * 0.5 + 0.5 - distance).clamp(0.0, 1.0);
                    if coverage > 0.0 {
                        blend(&mut self.pixels[index], rgb, (coverage * 255.0) as u32);
                    }
                }
            }
        }
    }

    fn screen(&mut self, screen: &Screen, dy: i32, name_elapsed: Duration) -> Painted {
        let Layout { width, height, chart_w, values_w, node_w, gap, .. } = self.layout;
        let (top, bottom) = (height / 4 + dy, height * 3 / 4 + dy);
        let node_x = width - node_w;
        let palette = self.palette;
        match screen {
            Screen::NotRunning | Screen::NoListeners => {
                let label = if matches!(screen, Screen::NotRunning) { "未运行" } else { "无监听" };
                self.text(label, node_x, height / 2 + dy, (node_x, width), palette.dim, 0);
                Painted { scroll_done: true, name_width: 0 }
            }
            Screen::Listener(view) => {
                self.chart(&view.series, dy);
                let values_x = chart_w + gap;
                let clip = (values_x, values_x + values_w);
                self.text(&format_rate(view.up), values_x, top, clip, palette.up, 0);
                self.text(&format_rate(view.down), values_x, bottom, clip, palette.down, 0);
                let name: Vec<u16> = view.node.encode_utf16().collect();
                let name_width = self.text_size(&name).cx;
                let (offset, scroll_done) = scroll_offset(name_width, node_w, name_elapsed);
                self.text(&view.node, node_x, top, (node_x, width), palette.text, offset);
                let (delay, rgb) = match view.delay {
                    DelayView::Ms(ms, tone) => (
                        format!("● {ms}"),
                        match tone {
                            Tone::Good => palette.good,
                            Tone::Warn => palette.warn,
                            Tone::Bad => palette.bad,
                        },
                    ),
                    DelayView::Failed => ("● --".to_string(), palette.bad),
                    DelayView::Unknown => ("● --".to_string(), palette.dim),
                };
                self.text(&delay, node_x, bottom, (node_x, width), rgb, 0);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        })
    }

    fn frame(screen: Screen) -> Frame {
        Frame { screen, previous: None, slide_progress: 1.0, name_elapsed: Duration::ZERO, light: false }
    }

    fn painted(frame: &Frame) -> (Vec<u32>, Painted) {
        let layout = layout(96);
        let mut pixels = vec![0; (layout.width * layout.height) as usize];
        let done = paint(frame, &layout, &mut pixels);
        (pixels, done)
    }

    #[test]
    fn layout_scales_with_dpi() {
        let one = layout(96);
        assert!((90..=100).contains(&one.width));
        assert_eq!((one.node_w, one.chart_w), (36, 26));
        let two = layout(192);
        assert_eq!((two.width, two.height, two.node_w, two.chart_w), (one.width * 2, one.height * 2, 72, 52));
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
    fn paint_draws_chart_and_text() {
        let (pixels, _) = painted(&frame(listener_screen()));
        let layout = layout(96);
        let column = |range: std::ops::Range<i32>| {
            pixels
                .chunks(layout.width as usize)
                .any(|row| range.clone().any(|x| row[x as usize] >> 24 > 1))
        };
        assert!(column(0..layout.chart_w));
        assert!(column(layout.chart_w..layout.width - layout.node_w));
        assert!(column(layout.width - layout.node_w..layout.width));
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
        let (pixels, _) = painted(&frame(screen));
        let layout = layout(96);
        rows.flat_map(|y| (0..layout.chart_w).map(move |x| (y * layout.width + x) as usize))
            .any(|i| pixels[i] >> 24 > 1)
    }

    #[test]
    fn chart_is_empty_without_traffic() {
        // A flat line on the bottom edge reads as another app's running indicator.
        assert!(!chart_inked(listener_with(vec![Rate::default(); 10]), 0..layout(96).height));
    }

    #[test]
    fn chart_stays_clear_of_the_taskbar_running_indicator() {
        let h = layout(96).height;
        let series = (0..10).map(|i| Rate { up: 0, down: i % 2 * 1000 }).collect();
        assert!(chart_inked(listener_with(series), 0..h));
        let series = (0..10).map(|i| Rate { up: 0, down: i % 2 * 1000 }).collect();
        assert!(!chart_inked(listener_with(series), h - 4..h));
    }

    #[test]
    fn paint_reports_the_node_name_width() {
        assert!(painted(&frame(listener_screen())).1.name_width > layout(96).node_w);
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
        assert_eq!(palette(false).up, 0xfbbf24);
        assert_eq!(palette(false).down, 0x38bdf8);
    }
}
