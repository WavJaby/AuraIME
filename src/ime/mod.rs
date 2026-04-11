mod helpers;
mod java_access_bridge;

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Accessibility::{
    UIA_ComboBoxControlTypeId, UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_IsPasswordPropertyId,
};
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImeStatus {
    pub hwnd: isize,
    pub display_name: String,
    pub is_open: bool,
    pub has_caret: bool,
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
    has_caret: bool,
}

static LAST_STATE: Mutex<LastImeState> =
    Mutex::new(LastImeState { hwnd: 0, hkl: 0, is_open: false, conv_mode: 0, has_caret: false });

fn update_ime_status(hwnd: HWND, hkl: HKL, is_open: bool, conv_mode: u32, has_caret: bool) -> bool {
    let mut last_state = LAST_STATE.lock().unwrap();
    let hwnd_isize = hwnd.0 as isize;
    if last_state.hwnd == hwnd_isize
        && last_state.hkl == hkl.0 as usize
        && last_state.is_open == is_open
        && last_state.conv_mode == conv_mode
        && last_state.has_caret == has_caret
    {
        return false;
    }
    *last_state = LastImeState { hwnd: hwnd_isize, hkl: hkl.0 as usize, is_open, conv_mode, has_caret };
    true
}

fn is_cjk_lang(lang_id: u16) -> bool {
    let primary = lang_id & 0x3ff;
    matches!(primary, 0x04 | 0x11 | 0x12)
}

fn get_window_class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];
    unsafe {
        let len = GetClassNameW(hwnd, &mut buffer);
        if len > 0 {
            // convert UTF-16 to String
            let os_string = OsString::from_wide(&buffer[..len as usize]);
            os_string.into_string().ok()
        } else {
            None
        }
    }
}

fn is_blacklisted_window(hwnd: HWND) -> bool {
    if let Some(class_name) = get_window_class_name(hwnd) {
        // Common system window class names without input fields
        let blacklist = [
            "Shell_TrayWnd", // Windows taskbar main body
            "TopLevelWindowForOverflowXamlIsland",
            "TrayNotifyWnd",              // System tray in the lower right corner
            "WorkerW",                    // Desktop (modern Windows desktop background)
            "Progman",                    // Desktop (legacy Program Manager)
            "Windows.UI.Core.CoreWindow", // Some UWP base containers (optional depending on context)
            "Xaml_WindowedPopupClass",    // System popup menus (such as volume control, etc.)
        ];

        blacklist.contains(&class_name.as_str())
    } else {
        false
    }
}

fn is_chromium_window(hwnd: HWND) -> bool {
    if let Some(class_name) = get_window_class_name(hwnd) {
        class_name == "Chrome_WidgetWin_1"
            || class_name == "Chrome_RenderWidgetHostHWND"
            || class_name == "CefBrowserWindow" // CEF (Chromium Embedded Framework)
    } else {
        false
    }
}

pub fn is_chromium_input_focused(hwnd: HWND) -> bool {
    if !is_chromium_window(hwnd) {
        return true;
    }

    unsafe {
        // Force wake up Chromium's accessibility tree
        const OBJID_CLIENT: u32 = 0xFFFFFFFC;
        SendMessageW(hwnd, WM_GETOBJECT, WPARAM(0).into(), LPARAM(OBJID_CLIENT as isize).into());

        let Some(uia) = crate::monitor::caret::get_uia_instance() else {
            return false;
        };

        // Get the currently focused UI element
        let Ok(element) = uia.GetFocusedElement() else {
            return false;
        };

        // Check the control type
        let Ok(control_type) = element.CurrentControlType() else {
            return false;
        };

        println!("[IME] Control type: {:?}", control_type);
        // UIA_EditControlTypeId // General plain text box
        // UIA_DocumentControlTypeId // Complex editing area (Notion, VSCode)
        // UIA_ComboBoxControlTypeId // Input box with dropdown suggestions

        if control_type != UIA_EditControlTypeId
            && control_type != UIA_DocumentControlTypeId
            && control_type != UIA_ComboBoxControlTypeId
        {
            return false;
        }

        if let Ok(variant) = element.GetCurrentPropertyValue(UIA_IsPasswordPropertyId)
            && bool::try_from(&variant).unwrap_or(false)
        {
            return false;
        }

        true
    }
}

pub fn get_status_from_hwnd(hwnd: HWND) -> Option<ImeStatus> {
    if hwnd.is_invalid() {
        return None;
    }

    if is_blacklisted_window(hwnd) {
        return None;
    }

    unsafe {
        let tid = GetWindowThreadProcessId(hwnd, None);
        let hkl = GetKeyboardLayout(tid);

        let is_open;
        let conv_mode;

        // Check if caret is visible
        let mut has_caret = true;
        if has_caret {
            has_caret = is_chromium_input_focused(hwnd);
        }

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
        if !update_ime_status(hwnd, hkl, is_open, conv_val, has_caret) {
            return None;
        }

        let lang_info = helpers::get_lang_info(hkl)?;

        let status_name = helpers::get_locate_language(lang_info.main).unwrap_or_else(|| "Unknown".to_string());

        let cjk_lang = is_cjk_lang(lang_info.main);

        println!(
            "[IME] State changed - hwnd: {:?},hkl: {:08X}, name: {},  is_open: {}, mode: {:x}, cjk_lang: {}, has_caret: {}",
            hwnd, hkl.0 as usize, status_name, is_open, conv_val, cjk_lang, has_caret
        );

        // Not enable
        if !cjk_lang && !is_open {
            return None;
        }

        Some(ImeStatus {
            hwnd: hwnd.0 as isize,
            display_name: status_name,
            is_open,
            has_caret,
            conv_mode: conv_val,
            cjk_lang,
            lang_id: lang_info.main,
        })
    }
}
