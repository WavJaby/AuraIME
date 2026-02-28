use crate::ui::accent;
use crate::ui::animation::{AnimationPhase, AnimationState};
use crate::ui::layout::LayoutManager;
use crate::ui::part::ImePart;
use crate::ui::renderer::UiRenderer;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::Ime::IME_CMODE_NATIVE;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_USER_UPDATE_POSITION: u32 = WM_USER + 1;
const WM_USER_SHOW_AND_FADE: u32 = WM_USER + 2;
const TIMER_ID_FADE: usize = 1;

pub struct OverlayWindow {
    pub hwnd: HWND,
    pub dwrite_factory: IDWriteFactory,
    pub renderer: Arc<Mutex<UiRenderer>>,
    pub text_format: IDWriteTextFormat,
    pub parts: Arc<Mutex<Vec<ImePart>>>,
    pub current_status: Arc<Mutex<Option<crate::ime::ImeStatus>>>,
    pub animation: Arc<Mutex<AnimationState>>,
}

unsafe impl Send for OverlayWindow {}
unsafe impl Sync for OverlayWindow {}

fn set_dwm_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) {
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, attribute, value as *const T as *const _, size_of::<T>() as u32);
    }
}

impl OverlayWindow {
    pub fn new() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let window_class = w!("AuraIME_Overlay");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: instance.into(),
                lpszClassName: window_class,
                hbrBackground: HBRUSH::default(),
                ..Default::default()
            };

            RegisterClassW(&wc);

            let d2d_factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            let text_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI Variable Text"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                w!("en-US"),
            )?;

            text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let initial_status = crate::ime::ImeStatus {
                hwnd: 0,
                display_name: "Loading".to_string(),
                is_open: false,
                conv_mode: 0,
                has_other_modes: false,
                lang_id: 0x0409,
            };
            let initial_part = ImePart::new(&initial_status.display_name, &dwrite_factory, &text_format, 32.0)?;

            let current_status = Arc::new(Mutex::new(Some(initial_status)));
            let renderer = Arc::new(Mutex::new(UiRenderer::new(d2d_factory.clone())));
            let parts = Arc::new(Mutex::new(vec![initial_part]));
            let animation = Arc::new(Mutex::new(AnimationState::new()));

            let overlay = Box::new(Self {
                hwnd: HWND::default(),
                dwrite_factory: dwrite_factory.clone(),
                renderer: renderer.clone(),
                text_format: text_format.clone(),
                parts: parts.clone(),
                current_status: current_status.clone(),
                animation: animation.clone(),
            });
            let overlay_ptr = overlay.as_ref() as *const _ as *const _;

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                window_class,
                w!("AuraIME"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                150,
                32,
                None,
                None,
                Some(instance.into()),
                Some(overlay_ptr),
            )?;

            let overlay_ptr = Box::into_raw(overlay);
            (*overlay_ptr).hwnd = hwnd;

            let hwnd_isize = hwnd.0 as isize;
            std::thread::spawn(move || {
                let hwnd = HWND(hwnd_isize as *mut _);
                loop {
                    if !IsWindow(Some(hwnd)).as_bool() {
                        break;
                    }

                    if let Err(_) = DwmFlush() {
                        std::thread::sleep(Duration::from_millis(16));
                    }
                    if IsWindowVisible(hwnd).as_bool() {
                        let _ = PostMessageW(Some(hwnd), WM_USER_UPDATE_POSITION, WPARAM(0), LPARAM(0));
                    }
                }
            });

            Self::setup_modern_look(hwnd);

            let final_overlay = Self {
                hwnd,
                dwrite_factory: dwrite_factory.clone(),
                renderer,
                text_format: text_format.clone(),
                parts,
                current_status,
                animation,
            };

            Ok(final_overlay)
        }
    }

    pub fn setup_modern_look(hwnd: HWND) {
        // DwmExtendFrameIntoClientArea with -1 margins
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        unsafe {
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        }

        // Acrylic backdrop (Type 3)
        set_dwm_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &3u32);

        // Accent Policy (legacy Acrylic path)
        accent::set_accent_policy(hwnd, 4); // ACCENT_ENABLE_ACRYLICBLURBEHIND

        // Rounded corners (Win11)
        set_dwm_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND);

        // Dark mode
        set_dwm_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &BOOL(1));

        // // Force backdrop active when unfocused (undocumented 1029)
        // set_dwm_attribute(hwnd, unsafe { core::mem::transmute(1029i32) }, &BOOL(1));
        //
        // // Passive Update Mode (undocumented 1032)
        // set_dwm_attribute(hwnd, unsafe { core::mem::transmute(1032i32) }, &BOOL(1));

        // Exclude from peek
        set_dwm_attribute(hwnd, DWMWA_EXCLUDED_FROM_PEEK, &BOOL(1));
    }

    pub fn update_status(&self, status: crate::ime::ImeStatus) -> Result<()> {
        let mut new_parts = vec![status.display_name.clone()];

        if status.has_other_modes {
            if !status.is_open {
                new_parts.push("A".to_string());
            } else if (status.conv_mode & IME_CMODE_NATIVE.0) == 0 {
                new_parts.push("A".to_string());
            } else {
                let indicator = match status.lang_id & 0x3ff {
                    0x11 => "あ",
                    0x12 => "한",
                    _ => "中",
                };
                new_parts.push(indicator.to_string());
            }
        }

        let mut current = self.current_status.lock().unwrap();
        if current.as_ref() != Some(&status) {
            let log_text = new_parts.join(" | ");
            *current = Some(status);
            drop(current);

            // Pre-generate cached ImePart layouts
            let mut cached = Vec::with_capacity(new_parts.len());
            for text in &new_parts {
                match ImePart::new(text, &self.dwrite_factory, &self.text_format, 32.0) {
                    Ok(part) => cached.push(part),
                    Err(e) => println!("[UI] Failed to create ImePart for '{}': {:?}", text, e),
                }
            }
            *self.parts.lock().unwrap() = cached;

            let mut anim_lock = self.animation.lock().unwrap();
            let skip_fade_in = anim_lock.update_on_status_change();
            if !skip_fade_in {
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_USER_SHOW_AND_FADE, WPARAM(0), LPARAM(0)) };
            }
            drop(anim_lock);

            println!("[UI] Status updated to: {}", log_text);
            unsafe {
                self.resize_to_content()?;
                println!("[UI] InvalidateRect calling...");
                let _ = InvalidateRect(Some(self.hwnd), None, true);
                println!("[UI] InvalidateRect called.");
            }
        }
        Ok(())
    }

    pub fn resize_to_content(&self) -> Result<()> {
        let parts = self.parts.lock().unwrap();
        let total_width = LayoutManager::total_width(&parts);
        drop(parts);

        println!("[UI] SetWindowPos calling... width: {}", total_width);
        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                total_width.ceil() as i32,
                32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        println!("[UI] SetWindowPos called.");
        Ok(())
    }

    fn update_position_hwnd(hwnd: HWND) -> Result<()> {
        let overlay = match Self::get_overlay(hwnd) {
            Some(o) => o,
            None => return Ok(()),
        };

        // Fade in animation
        let (alpha, vertical_offset) = {
            let anim = overlay.animation.lock().unwrap();
            anim.get_alpha_and_offset()
        };

        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), (alpha * 255.0) as u8, LWA_ALPHA);

            let mut info = GUITHREADINFO::default();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(0, &mut info).is_ok() {
                let mut pt = if !info.hwndCaret.is_invalid() {
                    let mut pt = POINT {
                        x: info.rcCaret.left,
                        y: info.rcCaret.bottom,
                    };
                    let _ = ClientToScreen(info.hwndCaret, &mut pt);
                    pt
                } else {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    pt
                };

                pt.y += 20 + vertical_offset as i32;
                SetWindowPos(hwnd, Some(HWND_TOPMOST), pt.x, pt.y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE)?;
            }
        }
        Ok(())
    }

    fn on_create(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        println!("[UI] WM_CREATE");
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        let overlay = unsafe { (*create_struct).lpCreateParams as *mut OverlayWindow };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, overlay as isize);
        }
        LRESULT(0)
    }

    fn on_show_and_fade(hwnd: HWND) -> LRESULT {
        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetTimer(Some(hwnd), TIMER_ID_FADE, 16, None);
        }
        LRESULT(0)
    }

    fn on_vsync(hwnd: HWND) -> LRESULT {
        Self::update_position_hwnd(hwnd).expect("Failed to update position");
        LRESULT(0)
    }

    fn on_paint(hwnd: HWND, overlay: &OverlayWindow) -> LRESULT {
        println!("[UI] WM_PAINT");

        let mut ps = PAINTSTRUCT::default();
        let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rect);
        }
        let height = (rect.bottom - rect.top) as f32;

        {
            let mut renderer = overlay.renderer.lock().unwrap();
            if let Err(e) = renderer.ensure_target(hwnd) {
                println!("[UI] Failed to create RenderTarget: {:?}", e);
            } else {
                let status_lock = overlay.current_status.lock().unwrap();
                if let Some(status) = status_lock.as_ref() {
                    let parts = overlay.parts.lock().unwrap();
                    println!("[UI] Drawing {} parts", parts.len());
                    renderer.draw_frame(status, &parts, height);
                }
            }
        }

        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        LRESULT(0)
    }

    fn on_size(overlay: &OverlayWindow, lparam: LPARAM) -> LRESULT {
        let width = (lparam.0 & 0xFFFF) as u32;
        let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
        let renderer = overlay.renderer.lock().unwrap();
        renderer.resize(width, height);
        LRESULT(0)
    }

    fn on_ncdestroy(hwnd: HWND) -> LRESULT {
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER_ID_FADE);
        }
        let overlay = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut OverlayWindow };
        if !overlay.is_null() {
            let _ = unsafe { Box::from_raw(overlay) };
        }
        LRESULT(0)
    }

    fn on_timer(hwnd: HWND, overlay: &OverlayWindow, timer_id: usize) -> LRESULT {
        if timer_id == TIMER_ID_FADE {
            let phase = {
                let anim = overlay.animation.lock().unwrap();
                anim.get_phase()
            };

            if phase == AnimationPhase::Finished {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let _ = KillTimer(Some(hwnd), TIMER_ID_FADE);
                }
            }
        }
        LRESULT(0)
    }

    #[inline]
    fn get_overlay<'a>(hwnd: HWND) -> Option<&'a Self> {
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayWindow };
        if ptr.is_null() { None } else { Some(unsafe { &*ptr }) }
    }

    pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => Self::on_create(hwnd, lparam),
            // WM_NCACTIVATE => Self::on_nc_activate(hwnd, msg, wparam, lparam),
            // WM_ERASEBKGND => LRESULT(1),
            WM_USER_UPDATE_POSITION => Self::on_vsync(hwnd),
            WM_USER_SHOW_AND_FADE => Self::on_show_and_fade(hwnd),
            WM_TIMER => match Self::get_overlay(hwnd) {
                Some(overlay) => Self::on_timer(hwnd, overlay, wparam.0),
                None => LRESULT(0),
            },
            WM_PAINT => match Self::get_overlay(hwnd) {
                Some(overlay) => Self::on_paint(hwnd, overlay),
                None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            },
            WM_SIZE => {
                if let Some(overlay) = Self::get_overlay(hwnd) {
                    Self::on_size(overlay, lparam);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => Self::on_ncdestroy(hwnd),
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
