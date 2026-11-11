//! 本机已安装应用的图标：从商店包 / exe 取系统显示的那个图标，转成 PNG data URI。
//! 列表里的 `icon` 只是占位，拿不到本机图标时才用它。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::detect::LaunchTarget;

/// 取图尺寸（px）。卡片显示 48px，按 2 倍屏取。
#[cfg(windows)]
const ICON_PX: i32 = 96;

/// Shell 解析名：商店应用走 AppsFolder，exe 用路径本身。
pub fn parsing_name(target: &LaunchTarget) -> String {
    match target {
        LaunchTarget::Aumid(aumid) => format!("shell:AppsFolder\\{aumid}"),
        LaunchTarget::Exe(path) => path.to_string_lossy().into_owned(),
    }
}

/// 本机图标的 data URI；取不到为 None。按解析名缓存（含失败），每次列表刷新不重复取图。
pub fn local_icon(target: &LaunchTarget) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let key = parsing_name(target);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return hit.clone();
    }
    let icon = extract(&key).map(|png| {
        use base64::Engine;
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
    });
    if icon.is_none() {
        eprintln!("aiapps: no local icon for {key}");
    }
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, icon.clone());
    icon
}

/// Shell 给出的 32 位位图是否为预乘 alpha：预乘时每个颜色分量都不超过 alpha。
fn premultiplied(bgra: &[u8]) -> bool {
    bgra.chunks_exact(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3])
}

/// BGRA（可能预乘、可能整张无 alpha）→ 直通 alpha 的 RGBA。
fn to_rgba(mut px: Vec<u8>) -> Vec<u8> {
    let no_alpha = px.chunks_exact(4).all(|p| p[3] == 0);
    let unpremultiply = !no_alpha && premultiplied(&px);
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
        if no_alpha {
            p[3] = 255;
        } else if unpremultiply && p[3] > 0 && p[3] < 255 {
            let a = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    px
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().ok()?;
    w.write_image_data(rgba).ok()?;
    w.finish().ok()?;
    Some(out)
}

#[cfg(windows)]
fn extract(parsing_name: &str) -> Option<Vec<u8>> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, IBindCtx, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY};

    unsafe {
        // 线程已是 MTA 时返回 RPC_E_CHANGED_MODE：照样能取图，只是不能配对反初始化。
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None::<&IBindCtx>).ok()?;
            let bmp = factory.GetImage(SIZE { cx: ICON_PX, cy: ICON_PX }, SIIGBF_ICONONLY).ok()?;
            let mut info = BITMAP::default();
            let got = GetObjectW(bmp.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut info as *mut _ as *mut _));
            let (w, h) = (info.bmWidth, info.bmHeight.abs());
            let px = (got > 0 && w > 0 && h > 0).then(|| {
                let mut bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h, // 自上而下
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut buf = vec![0u8; (w * h * 4) as usize];
                let dc = GetDC(None);
                let lines = GetDIBits(dc, bmp, 0, h as u32, Some(buf.as_mut_ptr().cast()), &mut bi, DIB_RGB_COLORS);
                ReleaseDC(None, dc);
                (lines == h).then_some(buf)
            });
            let _ = DeleteObject(bmp.into());
            encode_png(w as u32, h as u32, &to_rgba(px.flatten()?))
        })();
        if com {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
fn extract(_parsing_name: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parsing_names() {
        assert_eq!(parsing_name(&LaunchTarget::Aumid("F_x!App".into())), "shell:AppsFolder\\F_x!App");
        assert_eq!(parsing_name(&LaunchTarget::Exe(PathBuf::from("C:\\a\\b.exe"))), "C:\\a\\b.exe");
    }

    #[test]
    fn rgba_from_premultiplied_bgra() {
        // 半透明纯红（预乘：R=128,A=128）与不透明蓝。
        let out = to_rgba(vec![0, 0, 128, 128, 255, 0, 0, 255]);
        assert_eq!(out, vec![255, 0, 0, 128, 0, 0, 255, 255]);
    }

    #[test]
    fn rgba_from_straight_bgra_is_kept() {
        // 颜色分量大于 alpha：不是预乘，原样保留。
        assert_eq!(to_rgba(vec![10, 20, 200, 100]), vec![200, 20, 10, 100]);
    }

    #[test]
    fn rgba_without_alpha_becomes_opaque() {
        assert_eq!(to_rgba(vec![1, 2, 3, 0, 4, 5, 6, 0]), vec![3, 2, 1, 255, 6, 5, 4, 255]);
    }

    #[test]
    fn encodes_png() {
        let png = encode_png(1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    /// 本机检查：把已安装列表应用的图标写到临时目录下的 routa-ai-icons。
    #[test]
    #[ignore]
    fn icons_real_machine() {
        use base64::Engine;
        let dir = std::env::temp_dir().join("routa-ai-icons");
        std::fs::create_dir_all(&dir).unwrap();
        for app in crate::aiapps::bundled().apps {
            let icon = crate::aiapps::detect::detect(&app).and_then(|t| local_icon(&t));
            println!("{}: {}", app.id, icon.as_ref().map_or(0, |s| s.len()));
            if let Some(b64) = icon.as_deref().and_then(|s| s.strip_prefix("data:image/png;base64,")) {
                let png = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
                std::fs::write(dir.join(format!("{}.png", app.id)), png).unwrap();
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn extracts_icon_of_a_system_exe() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let uri = local_icon(&LaunchTarget::Exe(PathBuf::from(format!("{windir}\\explorer.exe")))).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
    }
}
