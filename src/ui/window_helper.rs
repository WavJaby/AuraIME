use windows::core::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, DPI_AWARENESS_PER_MONITOR_AWARE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::Error;

pub fn get_monitor_work_area(rect: &RECT) -> Result<RECT> {
    unsafe {
        let monitor = MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return Err(Error::from_thread());
        }
        Ok(monitor_info.rcWork)
    }
}

pub fn set_window_pos_topmost(hwnd: HWND, x: i32, y: i32) -> Result<()> {
    unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE) }
}

pub fn get_window_dpi_scale(hwnd: HWND) -> Result<f32> {
    let dpi = unsafe { GetDpiForWindow(hwnd) } as i32;
    if dpi <= DPI_AWARENESS_PER_MONITOR_AWARE.0 {
        return Err(Error::from_thread());
    }
    Ok(dpi as f32 / 96.0)
}
