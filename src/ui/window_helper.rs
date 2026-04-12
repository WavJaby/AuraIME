use windows::core::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, DPI_AWARENESS_PER_MONITOR_AWARE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::Error;
pub fn init_window(wnd_proc: WNDPROC, overlay: *const OverlayWindow, window_class: PCWSTR, window_name: PCWSTR) -> Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None)?.into() };

    let wc = WNDCLASSW {
        lpfnWndProc: wnd_proc,
        hInstance: instance,
        lpszClassName: window_class,
        lpszMenuName: window_name,
        hbrBackground: HBRUSH::default(),
        ..Default::default()
    };

    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err(Error::from_thread());
    }

    unsafe {
        Ok(CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            window_class,
            window_name,
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            None,
            Some(instance),
            Some(overlay as *const _),
        )?)
    }
}

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

pub fn post_window_message(hwnd: HWND, msg: u32) -> Result<()> {
    unsafe { PostMessageW(Some(hwnd), msg, WPARAM(0), LPARAM(0)) }
}

pub fn set_window_pos_topmost(hwnd: HWND, x: i32, y: i32) -> Result<()> {
    unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOREDRAW) }
}

pub fn set_window_size(hwnd: HWND, width: i32, height: i32) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
        )
    }
}

pub fn get_window_dpi_scale(hwnd: HWND) -> Result<f32> {
    let dpi = unsafe { GetDpiForWindow(hwnd) } as i32;
    if dpi <= DPI_AWARENESS_PER_MONITOR_AWARE.0 {
        return Err(Error::from_thread());
    }
    Ok(dpi as f32 / 96.0)
}
