use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;

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

pub fn set_accent_policy(hwnd: HWND, accent_state: u32) {
    let accent = AccentPolicy {
        accent_state,
        accent_flags: 0,
        gradient_color: 0x00000000,
        animation_id: 0,
    };

    let data = WindowCompositionAttribData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: &accent as *const _ as *const _,
        size_of_data: std::mem::size_of::<AccentPolicy>(),
    };

    if let Ok(user32) = unsafe { GetModuleHandleW(w!("user32.dll")) } {
        let addr = unsafe { GetProcAddress(user32, s!("SetWindowCompositionAttribute")) };
        if let Some(addr) = addr {
            let func: SetWindowCompositionAttribute = unsafe { std::mem::transmute(addr) };
            unsafe {
                let _ = func(hwnd, &data);
            }
        }
    }
}
