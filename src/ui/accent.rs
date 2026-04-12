use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_EXCLUDED_FROM_PEEK, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWINDOWATTRIBUTE};
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::MARGINS;

#[repr(C)]
pub struct AccentPolicy {
    pub accent_state: u32,
    pub accent_flags: u32,
    pub gradient_color: u32,
    pub animation_id: u32,
}

#[repr(C)]
pub struct WindowCompositionAttribData {
    pub attribute: u32,
    pub data: *const std::ffi::c_void,
    pub size_of_data: usize,
}

type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *const WindowCompositionAttribData) -> BOOL;

pub fn set_accent_policy(hwnd: HWND, accent_state: u32) -> Result<()> {
    let accent = AccentPolicy {
        accent_state,
        accent_flags: 0,
        gradient_color: 0x00000000,
        animation_id: 0,
    };

    let data = WindowCompositionAttribData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: &accent as *const _ as *const _,
        size_of_data: size_of::<AccentPolicy>(),
    };

    if let Ok(user32) = unsafe { GetModuleHandleW(w!("user32.dll")) } {
        let addr = unsafe { GetProcAddress(user32, s!("SetWindowCompositionAttribute")) };
        if let Some(addr) = addr {
            let func: SetWindowCompositionAttribute = unsafe { std::mem::transmute(addr) };
            if !unsafe { func(hwnd, &data).into() } {
                return Err(Error::from_thread());
            }
        }
    }

    Ok(())
}

pub fn set_dwm_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> Result<()> {
    unsafe { DwmSetWindowAttribute(hwnd, attribute, value as *const T as *const _, size_of::<T>() as u32) }
}

pub fn setup_modern_look(hwnd: HWND) -> Result<()> {
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins)? };

    // Acrylic backdrop (Type 3)
    set_dwm_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &3u32)?;

    // Accent Policy (legacy Acrylic path)
    set_accent_policy(hwnd, 4)?; // ACCENT_ENABLE_ACRYLICBLURBEHIND

    // Rounded corners (Win11)
    set_dwm_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND)?;

    // Dark mode
    set_dwm_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &BOOL(1))?;

    // // Force backdrop active when unfocused (undocumented 1029)
    // set_dwm_attribute(hwnd, unsafe { core::mem::transmute(1029i32) }, &BOOL(1));
    //
    // // Passive Update Mode (undocumented 1032)
    // set_dwm_attribute(hwnd, unsafe { core::mem::transmute(1032i32) }, &BOOL(1));

    // Exclude from peek
    set_dwm_attribute(hwnd, DWMWA_EXCLUDED_FROM_PEEK, &BOOL(1))?;

    Ok(())
}