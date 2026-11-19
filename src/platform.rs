//! Small Win32 helpers shared by the overlay thread, the GUI and `main`.

use std::ffi::c_void;
use std::path::Path;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, BOOL, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, POINT, RECT, TRUE,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetDC, GetMonitorInfoW, GetPixel, ReleaseDC, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

use crate::core::MonitorInfo;

/// Title of the settings window; also used to find it from other threads / instances.
pub const MAIN_TITLE: &str = "Crosshair reticle";

/// Per-monitor DPI v2, so every coordinate we deal with is a physical pixel.
pub fn set_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Keeps the single-instance mutex alive; dropping it (process exit) releases it.
pub struct InstanceGuard(#[allow(dead_code)] HANDLE);

/// `None` means another instance already owns the mutex.
pub fn single_instance() -> Option<InstanceGuard> {
    unsafe {
        match CreateMutexW(None, true, w!("Local\\Reticle.SingleInstance")) {
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    None
                } else {
                    Some(InstanceGuard(h))
                }
            }
            // cannot tell - better to run than to refuse to start
            Err(_) => Some(InstanceGuard(HANDLE::default())),
        }
    }
}

/// Shows and focuses the settings window of the running instance. Never call this while
/// holding the `Core` lock: the call is synchronous towards the GUI thread.
pub fn show_settings_window() -> bool {
    unsafe {
        let title: Vec<u16> = MAIN_TITLE
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) else {
            return false;
        };
        let _ = ShowWindow(
            hwnd,
            if IsIconic(hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );
        let _ = SetForegroundWindow(hwnd);
        true
    }
}

unsafe extern "system" fn monitor_cb(
    hmon: HMONITOR,
    _hdc: HDC,
    _rc: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let list = &mut *(data.0 as *mut Vec<MonitorInfo>);
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        let len = mi
            .szDevice
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(mi.szDevice.len());
        let r = mi.monitorInfo.rcMonitor;
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        list.push(MonitorInfo {
            name: String::from_utf16_lossy(&mi.szDevice[..len]),
            left: r.left,
            top: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
            primary: mi.monitorInfo.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
            dpi: dx,
        });
    }
    TRUE
}

/// All attached monitors in physical pixels, ordered by device name.
pub fn enum_monitors() -> Vec<MonitorInfo> {
    let mut list: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_cb),
            LPARAM(&mut list as *mut _ as isize),
        );
    }
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Colour of the screen pixel under the cursor (for the eyedropper).
pub fn cursor_pixel() -> Option<[u8; 3]> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return None;
        }
        let c = GetPixel(hdc, pt.x, pt.y);
        ReleaseDC(None, hdc);
        if c.0 == 0xFFFF_FFFF {
            return None;
        }
        Some([
            (c.0 & 0xFF) as u8,
            ((c.0 >> 8) & 0xFF) as u8,
            ((c.0 >> 16) & 0xFF) as u8,
        ])
    }
}

pub fn left_button_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

pub fn right_button_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

pub fn escape_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 }
}

pub fn open_folder(path: &Path) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

#[allow(dead_code)]
fn _assert_c_void(_: *mut c_void) {}
