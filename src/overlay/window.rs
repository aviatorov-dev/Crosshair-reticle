//! The click-through, always-on-top layered window that shows the crosshair.

use anyhow::{anyhow, Result};
use tiny_skia::Pixmap;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, UpdateLayeredWindow, HTTRANSPARENT, HWND_TOPMOST, MA_NOACTIVATE, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_APP, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_SETTINGCHANGE, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// Posted to the overlay window when the Controller has queued a command.
pub const WM_APP_WAKE: u32 = WM_APP + 1;
/// Posted by the window procedure when the display configuration changed.
pub const WM_APP_DISPLAY: u32 = WM_APP + 2;

pub const OVERLAY_TITLE: &str = "Crosshair reticle overlay";

unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        // These are *sent* messages. Turn them into queued ones so the message loop handles
        // them with plain `&mut self` access and no re-entrancy.
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
            let _ = PostMessageW(hwnd, WM_APP_DISPLAY, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub struct OverlayWindow {
    pub hwnd: HWND,
}

impl OverlayWindow {
    pub fn create() -> Result<Self> {
        unsafe {
            let hinst = GetModuleHandleW(None)?;
            let class = w!("CrosshairReticleOverlayWindow");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(overlay_proc),
                hInstance: hinst.into(),
                lpszClassName: class,
                ..Default::default()
            };
            // returns 0 if the class already exists (thread restart) - that is fine
            RegisterClassW(&wc);
            let title: Vec<u16> = OVERLAY_TITLE
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW,
                class,
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                hinst,
                None,
            )?;
            Ok(OverlayWindow { hwnd })
        }
    }

    /// Replaces the window contents (premultiplied RGBA pixmap) and moves it to `x, y`.
    pub fn present(&self, pm: &Pixmap, x: i32, y: i32) -> Result<()> {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(screen);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bmp = match CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(b) => b,
                Err(e) => {
                    let _ = DeleteDC(mem);
                    ReleaseDC(None, screen);
                    return Err(anyhow!("CreateDIBSection: {e}"));
                }
            };
            let old = SelectObject(mem, bmp);

            // premultiplied RGBA -> premultiplied BGRA
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            for (d, s) in dst.chunks_exact_mut(4).zip(pm.data().chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }

            let dst_pt = POINT { x, y };
            let src_pt = POINT { x: 0, y: 0 };
            let size = SIZE { cx: w, cy: h };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let res = UpdateLayeredWindow(
                self.hwnd,
                screen,
                Some(&dst_pt),
                Some(&size),
                mem,
                Some(&src_pt),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );

            SelectObject(mem, old);
            let _ = DeleteObject(bmp);
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            res.map_err(|e| anyhow!("UpdateLayeredWindow: {e}"))
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Re-asserts the topmost position without stealing focus (FR-07).
    pub fn raise(&self) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
