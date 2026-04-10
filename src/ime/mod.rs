mod helpers;

use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImeStatus {
    pub hwnd: isize,
    pub display_name: String,
    pub is_open: bool,
    pub conv_mode: u32,
    pub cjk_lang: bool,
    pub lang_id: u16,
}

#[derive(Default, PartialEq, Eq)]
struct LastImeState {
    hwnd: isize,
    hkl: usize,
    is_open: bool,
    conv_mode: u32,
}

static LAST_STATE: Mutex<LastImeState> = Mutex::new(LastImeState {
    hwnd: 0,
    hkl: 0,
    is_open: false,
    conv_mode: 0,
});

fn update_ime_status(hwnd: HWND, hkl: HKL, is_open: bool, conv_val: u32) -> bool {
    let mut last_state = LAST_STATE.lock().unwrap();
    let hwnd_isize = hwnd.0 as isize;
    if last_state.hwnd == hwnd_isize
        && last_state.hkl == hkl.0 as usize
        && last_state.is_open == is_open
        && last_state.conv_mode == conv_val
    {
        return false;
    }
    *last_state = LastImeState {
        hwnd: hwnd_isize,
        hkl: hkl.0 as usize,
        is_open,
        conv_mode: conv_val,
    };
    true
}

fn is_cjk_lang(lang_id: u16) -> bool {
    let primary = lang_id & 0x3ff;
    matches!(primary, 0x04 | 0x11 | 0x12)
}

pub fn get_status_from_hwnd(hwnd: HWND) -> Option<ImeStatus> {
    if hwnd.is_invalid() {
        return None;
    }

    unsafe {
        let tid = GetWindowThreadProcessId(hwnd, None);
        let hkl = GetKeyboardLayout(tid);

        let is_open;
        let conv_mode;

        // Use WM_IME_CONTROL to read
        let mut ime_hwnd = ImmGetDefaultIMEWnd(hwnd);
        if ime_hwnd.is_invalid() {
            ime_hwnd = ImmGetDefaultIMEWnd(GetForegroundWindow());
        }

        if !ime_hwnd.is_invalid() {
            conv_mode = helpers::get_conv_mode(ime_hwnd);
            is_open = helpers::get_open_status(ime_hwnd);
        } else {
            conv_mode = IME_CONVERSION_MODE(0);
            is_open = false;
        }

        let conv_val = conv_mode.0;

        // Check if state changed
        if !update_ime_status(hwnd, hkl, is_open, conv_val) {
            return None;
        }

        let lang_info = helpers::get_lang_info(hkl)?;

        let status_name = helpers::get_locate_language(lang_info.main).unwrap_or_else(|| "Unknown".to_string());

        let cjk_lang = is_cjk_lang(lang_info.main);

        println!(
            "[IME] State changed - hwnd: {:?}, is_open: {}, mode: {:x}, hkl: {:08X}, status_name: {}, cjk_lang: {}",
            hwnd, is_open, conv_val, hkl.0 as usize, status_name, cjk_lang
        );

        Some(ImeStatus {
            hwnd: hwnd.0 as isize,
            display_name: status_name,
            is_open,
            conv_mode: conv_val,
            cjk_lang,
            lang_id: lang_info.main,
        })
    }
}
