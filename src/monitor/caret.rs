use std::cell::RefCell;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    AccessibleObjectFromWindow, CUIAutomation, IAccessible, IUIAutomation, IUIAutomationTextPattern,
    IUIAutomationTextPattern2, IUIAutomationTextRange, TextUnit_Character, UIA_TextPattern2Id,
    UIA_TextPatternId,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetCompositionWindow, ImmGetContext, ImmReleaseContext, CFS_POINT, COMPOSITIONFORM,
};
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static UIA_INSTANCE: RefCell<Option<IUIAutomation>> = RefCell::new(None);
}

pub fn get_uia_instance() -> Option<IUIAutomation> {
    UIA_INSTANCE.with(|uia| {
        let mut uia_ref = uia.borrow_mut();
        if uia_ref.is_none() {
            unsafe {
                if let Ok(instance) = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                    *uia_ref = Some(instance);
                } else {
                    log::error!("Failed to create CUIAutomation.");
                }
            }
        }
        uia_ref.clone()
    })
}

pub fn get_caret_rect_from_uia(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let uia = get_uia_instance()?;

        // Try focused element first, then fall back to ElementFromHandle
        let element = uia.GetFocusedElement().or_else(|_| uia.ElementFromHandle(hwnd));
        let element = match element {
            Ok(v) => v,
            Err(_) => return None,
        };

        // Try TextPattern2 for GetCaretRange
        if let Ok(pat_obj) = element.GetCurrentPattern(UIA_TextPattern2Id) {
            if let Ok(pattern2) = pat_obj.cast::<IUIAutomationTextPattern2>() {
                let mut is_active = BOOL(0);
                if let Ok(range) = pattern2.GetCaretRange(&mut is_active) {
                    if let Some(rect) = rect_from_text_range(&range) {
                        return Some(rect);
                    }
                }
            }
        }

        // Fallback: TextPattern GetSelection
        if let Ok(pat_obj) = element.GetCurrentPattern(UIA_TextPatternId) {
            if let Ok(pattern1) = pat_obj.cast::<IUIAutomationTextPattern>() {
                if let Ok(ranges) = pattern1.GetSelection() {
                    if ranges.Length().unwrap_or(0) > 0 {
                        if let Ok(range) = ranges.GetElement(0) {
                            if let Some(rect) = rect_from_text_range(&range) {
                                return Some(rect);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

// Try GetBoundingRectangles on a text range; if the range is degenerate (zero-size),
// expand it to a character and retry once.
fn rect_from_text_range(range: &IUIAutomationTextRange) -> Option<RECT> {
    unsafe {
        if let Ok(sa) = range.GetBoundingRectangles() {
            if let Some(rect) = extract_rect_from_safe_array(sa) {
                return Some(rect);
            }
            // Degenerate range — expand to character and retry
            let _ = range.ExpandToEnclosingUnit(TextUnit_Character);
            if let Ok(sa2) = range.GetBoundingRectangles() {
                if let Some(rect) = extract_rect_from_safe_array(sa2) {
                    return Some(rect);
                }
            }
        }
        None
    }
}

// Use SafeArrayAccessData/UnaccessData for safe memory access
fn extract_rect_from_safe_array(sa: *mut windows::Win32::System::Com::SAFEARRAY) -> Option<RECT> {
    unsafe {
        if sa.is_null() {
            return None;
        }
        let lower = SafeArrayGetLBound(&*sa, 1).ok()?;
        let upper = SafeArrayGetUBound(&*sa, 1).ok()?;
        let elem_count = (upper - lower + 1) as usize;
        if elem_count < 4 {
            return None;
        }

        let mut data_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
        if SafeArrayAccessData(&*sa, &mut data_ptr).is_err() {
            return None;
        }

        let doubles = std::slice::from_raw_parts(data_ptr as *const f64, elem_count);
        let left = doubles[0] as i32;
        let top = doubles[1] as i32;
        let width = doubles[2] as i32;
        let height = doubles[3] as i32;
        let _ = SafeArrayUnaccessData(&*sa);

        if width == 0 && height == 0 {
            return None;
        }
        Some(RECT { left, top, right: left + width, bottom: top + height })
    }
}

pub fn get_caret_rect_from_ime(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let fg = GetForegroundWindow();
        let target = if fg.0.is_null() { hwnd } else { fg };
        let h_imc = ImmGetContext(target);
        if h_imc.0.is_null() {
            return None;
        }
        let mut comp_form = COMPOSITIONFORM::default();
        let ok = ImmGetCompositionWindow(h_imc, &mut comp_form);
        let _ = ImmReleaseContext(target, h_imc);
        // Only use ptCurrentPos when dwStyle has CFS_POINT set;
        // Java Swing/AWT may report CFS_RECT or CFS_DEFAULT with a meaningless ptCurrentPos
        if ok.as_bool() && (comp_form.dwStyle & CFS_POINT) != 0 {
            let mut pt = comp_form.ptCurrentPos;
            let _ = ClientToScreen(target, &mut pt);
            if pt.x != 0 || pt.y != 0 {
                return Some(RECT { left: pt.x, top: pt.y, right: pt.x + 1, bottom: pt.y + 20 });
            }
        }
        None
    }
}

pub fn get_caret_rect_from_msaa(hwnd: HWND) -> Option<RECT> {
    const OBJID_CARET: u32 = 0xFFFFFFF8u32;
    unsafe {
        let fg = GetForegroundWindow();
        let target = if fg.0.is_null() { hwnd } else { fg };
        let iid = <IAccessible as windows_core::Interface>::IID;
        let mut p_acc: *mut core::ffi::c_void = core::ptr::null_mut();
        if AccessibleObjectFromWindow(target, OBJID_CARET, &iid, &mut p_acc).is_ok() && !p_acc.is_null() {
            let acc = IAccessible::from_raw(p_acc);
            let var_child = VARIANT::from(0i32);
            let mut x = 0i32;
            let mut y = 0i32;
            let mut w = 0i32;
            let mut h = 0i32;
            if acc.accLocation(&mut x, &mut y, &mut w, &mut h, &var_child).is_ok() {
                if x != 0 || y != 0 {
                    return Some(RECT { left: x, top: y, right: x + w.max(1), bottom: y + h.max(1) });
                }
            }
        }

        // Secondary fallback: GUITHREADINFO with hwndCaret -> hwndFocus -> hwndActive
        // Java apps often don't expose OBJID_CARET but rcCaret via hwndFocus is valid
        let mut gui = GUITHREADINFO::default();
        gui.cbSize = size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(0, &mut gui).is_ok() {
            let target_hwnd = if !gui.hwndCaret.0.is_null() {
                gui.hwndCaret
            } else if !gui.hwndFocus.0.is_null() {
                gui.hwndFocus
            } else if !gui.hwndActive.0.is_null() {
                gui.hwndActive
            } else {
                return None;
            };

            if gui.rcCaret.left != 0 || gui.rcCaret.top != 0 {
                let mut pt = POINT { x: gui.rcCaret.left, y: gui.rcCaret.top };
                let _ = ClientToScreen(target_hwnd, &mut pt);
                if pt.x > -1000 && pt.y > -1000 {
                    let mut pt_bottom = POINT { x: gui.rcCaret.right, y: gui.rcCaret.bottom };
                    let _ = ClientToScreen(target_hwnd, &mut pt_bottom);
                    return Some(RECT { left: pt.x, top: pt.y, right: pt_bottom.x, bottom: pt_bottom.y });
                }
            }
        }

        None
    }
}

pub fn get_caret_rect(hwnd: HWND) -> Option<RECT> {
    unsafe {
        // GUITHREADINFO (Win32 caret — works for classic apps)
        // Pass thread ID 0 so the system picks the foreground thread automatically
        let mut gui = GUITHREADINFO::default();
        gui.cbSize = size_of::<GUITHREADINFO>() as u32;
        let gui_ok = GetGUIThreadInfo(0, &mut gui).is_ok();
        if gui_ok && !gui.hwndCaret.0.is_null() {
            let mut pt = POINT { x: gui.rcCaret.left, y: gui.rcCaret.top };
            let _ = ClientToScreen(gui.hwndCaret, &mut pt);
            let mut pt_bottom = POINT { x: gui.rcCaret.right, y: gui.rcCaret.bottom };
            let _ = ClientToScreen(gui.hwndCaret, &mut pt_bottom);
            // println!(
            //     "[caret] -> GUITHREADINFO: {:?}",
            //     RECT { left: pt.x, top: pt.y, right: pt_bottom.x, bottom: pt_bottom.y }
            // );

            let w = pt_bottom.x - pt.x;
            let h = pt_bottom.y - pt.y;
            if h < 5 {
                pt_bottom.y += 16;
            }
            return Some(RECT { left: pt.x, top: pt.y, right: pt_bottom.x, bottom: pt_bottom.y });
        }

        // UI Automation (Chrome / Edge / modern apps)
        if let Some(rect) = get_caret_rect_from_uia(hwnd) {
            // println!("[caret] -> UI Automation: {:?}", rect);
            return Some(rect);
        }

        // IME composition window position (CFS_POINT only)
        if let Some(rect) = get_caret_rect_from_ime(hwnd) {
            // println!("[caret] -> IME: {:?}", rect);
            return Some(rect);
        }

        // MSAA OBJID_CARET + GUITHREADINFO secondary fallback (Java apps)
        if let Some(rect) = get_caret_rect_from_msaa(hwnd) {
            // println!("[caret] -> MSAA: {:?}", rect);
            return Some(rect);
        }

        None
    }
}
