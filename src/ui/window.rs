use crate::ime::ImeStatus;
use crate::ui::accent;
use crate::ui::animation::{AnimationPhase, AnimationState};
use crate::ui::parts::{Container, Part, Renderable, TextPart};
use crate::ui::renderer::UiRenderer;
use std::sync::{Arc, Mutex};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::Ime::IME_CMODE_NATIVE;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_USER_UPDATE_POSITION: u32 = WM_USER + 1;
const WM_USER_SHOW_AND_FADE: u32 = WM_USER + 2;
const TIMER_ID_FADE: usize = 1;

#[derive(Clone)]
pub struct OverlayWindow {
    pub hwnd: HWND,
    pub dwrite_factory: IDWriteFactory,
    pub renderer: Arc<Mutex<UiRenderer>>,
    pub text_format: IDWriteTextFormat,
    pub renderable: Arc<Mutex<Container>>,
    pub current_status: Arc<Mutex<ImeStatus>>,
    pub animation: Arc<Mutex<AnimationState>>,
    pub last_state: Arc<Mutex<LastWindowState>>,
}

pub struct LastWindowState {
    pub alpha: u8,
    pub x: i32,
    pub y: i32,
    pub caret_rect: RECT,
}

impl LastWindowState {
    pub fn new() -> Self {
        Self {
            alpha: 0,
            x: 0,
            y: 0,
            caret_rect: RECT::default(),
        }
    }
}

unsafe impl Send for OverlayWindow {}
unsafe impl Sync for OverlayWindow {}

fn set_dwm_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) {
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, attribute, value as *const T as *const _, size_of::<T>() as u32);
    }
}

impl OverlayWindow {
    pub fn new() -> Result<Arc<Self>> {
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

            let initial_container = Container::empty();
            let initial_width = initial_container.outer_width().ceil() as i32;
            let initial_height = initial_container.outer_height().ceil() as i32;

            let overlay = Arc::new(Self {
                hwnd: HWND::default(),
                dwrite_factory: dwrite_factory.clone(),
                renderer: Arc::new(Mutex::new(UiRenderer::new(d2d_factory))),
                text_format: text_format.clone(),
                renderable: Arc::new(Mutex::new(initial_container)),
                current_status: Arc::new(Mutex::new(ImeStatus::default())),
                animation: Arc::new(Mutex::new(AnimationState::new())),
                last_state: Arc::new(Mutex::new(LastWindowState::new())),
            });

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                window_class,
                w!("AuraIME"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                initial_width,
                initial_height,
                None,
                None,
                Some(instance.into()),
                Some(Arc::as_ptr(&overlay) as *const _),
            )?;

            Self::setup_modern_look(hwnd);

            // let hwnd_isize = hwnd.0 as isize;
            // std::thread::spawn(move || {
            //     let hwnd = HWND(hwnd_isize as *mut _);
            //     loop {
            //         if !IsWindow(Some(hwnd)).as_bool() {
            //             break;
            //         }
            //
            //         if let Err(_) = DwmFlush() {
            //             std::thread::sleep(Duration::from_millis(16));
            //         }
            //         if IsWindowVisible(hwnd).as_bool() {
            //             let _ = PostMessageW(Some(hwnd), WM_USER_UPDATE_POSITION, WPARAM(0), LPARAM(0));
            //         }
            //     }
            // });

            Ok(overlay)
        }
    }

    pub fn setup_modern_look(hwnd: HWND) {
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

    pub fn move_to_caret(&self, rect: RECT) -> Result<()> {
        let changed = {
            let mut last_state = self.last_state.lock().unwrap();
            if (*last_state).caret_rect == rect {
                false
            } else {
                (*last_state).caret_rect = rect;
                true
            }
        };

        if !changed {
            return Ok(());
        }

        let mut anim_lock = self.animation.lock().unwrap();
        let skip_fade_in = anim_lock.on_activity();
        if !skip_fade_in {
            println!("[UI] Caret moved, showing window.");
            let _ = unsafe { PostMessageW(Some(self.hwnd), WM_USER_SHOW_AND_FADE, WPARAM(0), LPARAM(0)) };
        }
        drop(anim_lock);

        Ok(())
    }

    fn get_caret_rect_from_hwnd(hwnd: HWND) -> RECT {
        unsafe {
            let mut gui = GUITHREADINFO::default();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            let tid = GetWindowThreadProcessId(hwnd, None);
            if GetGUIThreadInfo(tid, &mut gui).is_ok() && !gui.hwndCaret.is_invalid() {
                let mut rect = gui.rcCaret;
                let mut pt = POINT {
                    x: rect.left,
                    y: rect.top,
                };
                let _ = ClientToScreen(gui.hwndCaret, &mut pt);
                rect.left = pt.x;
                rect.top = pt.y;

                let mut pt_bottom = POINT {
                    x: gui.rcCaret.right,
                    y: gui.rcCaret.bottom,
                };
                let _ = ClientToScreen(gui.hwndCaret, &mut pt_bottom);
                rect.right = pt_bottom.x;
                rect.bottom = pt_bottom.y;

                return rect;
            }

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            RECT {
                left: pt.x,
                top: pt.y,
                right: pt.x,
                bottom: pt.y,
            }
        }
    }

    pub fn update_status(&self, status: ImeStatus) -> Result<()> {
        let indicator = if status.cjk_lang {
            if !status.is_open || (status.conv_mode & IME_CMODE_NATIVE.0) == 0 {
                Some("A")
            } else {
                Some(match status.lang_id & 0x3ff {
                    0x11 => "あ",
                    0x12 => "한",
                    _ => "中",
                })
            }
        } else {
            None
        };

        let mut current = self.current_status.lock().unwrap();
        *current = status.clone();
        drop(current);

        let caret_rect = Self::get_caret_rect_from_hwnd(HWND(status.hwnd as *mut _));
        {
            let mut last_state = self.last_state.lock().unwrap();
            (*last_state).caret_rect = caret_rect;
        }

        let text_color = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut childs: Vec<Box<dyn Renderable>> = Vec::with_capacity(2);
        let base = TextPart::with_color(
            &status.display_name,
            &self.dwrite_factory,
            &self.text_format,
            text_color,
        )?;
        let base_container = Container::new_with_color(
            vec![base],
            8.0,
            4.0,
            4.0,
            8.0,
            D2D1_COLOR_F {
                r: 0.26,
                g: 0.26,
                b: 0.26,
                a: 0.2,
            },
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.1,
            },
        );

        childs.push(base_container);
        if let Some(ind) = indicator {
            childs.push(TextPart::with_color(
                ind,
                &self.dwrite_factory,
                &self.text_format,
                text_color,
            )?);
        }
        *self.renderable.lock().unwrap() = Container::new(childs, 8.0, 8.0, 8.0);

        {
            let mut anim_lock = self.animation.lock().unwrap();
            let skip_fade_in = anim_lock.on_activity();
            if !skip_fade_in {
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_USER_SHOW_AND_FADE, WPARAM(0), LPARAM(0)) };
            }
        }

        println!(
            "[UI] Status updated to: {} {}",
            status.display_name,
            indicator.unwrap_or("")
        );
        unsafe {
            self.resize_to_content()?;
            let _ = InvalidateRect(Some(self.hwnd), None, true);
            println!("[UI] InvalidateRect");
        }

        Ok(())
    }

    pub fn resize_to_content(&self) -> Result<()> {
        let r = self.renderable.lock().unwrap();
        let total_width = r.outer_width();
        let total_height = r.outer_height();
        drop(r);

        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                total_width.ceil() as i32,
                total_height.ceil() as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        Ok(())
    }

    fn update_position(&self, hwnd: HWND) -> LRESULT {
        let (alpha_f, vertical_offset) = {
            let anim = self.animation.lock().unwrap();
            anim.get_alpha_and_offset()
        };
        let alpha = (alpha_f * 255.0) as u8;

        unsafe {
            let mut last = self.last_state.lock().unwrap();
            if last.alpha != alpha {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                last.alpha = alpha;
            }

            if last.caret_rect.left != 0 || last.caret_rect.top != 0 {
                let dpi = GetDpiForWindow(hwnd);
                let scale = dpi as f32 / 96.0;
                let offset_y = ((20.0 + vertical_offset) * scale) as i32;

                let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let vcx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                let vcy = GetSystemMetrics(SM_CYVIRTUALSCREEN);

                let x = last.caret_rect.left.clamp(vx, vx + vcx);
                let y = last.caret_rect.bottom + offset_y.clamp(vy, vy + vcy);

                if last.x != x || last.y != y {
                    SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE)
                        .expect("SetWindowPos failed");
                    last.x = x;
                    last.y = y;
                }
            }
        }
        LRESULT(0)
    }

    fn on_create(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        println!("[UI] WM_CREATE");
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        let overlay_ptr = unsafe { (*create_struct).lpCreateParams as *mut OverlayWindow };

        unsafe { (*overlay_ptr).hwnd = hwnd };
        let overlay = unsafe { Arc::from_raw(overlay_ptr) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::into_raw(overlay) as isize) };

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

    fn on_paint(hwnd: HWND, overlay: &OverlayWindow) -> LRESULT {
        println!("[UI] WM_PAINT");

        let mut ps = PAINTSTRUCT::default();
        let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };
        {
            let mut renderer = overlay.renderer.lock().unwrap();
            if let Err(e) = renderer.ensure_target(hwnd) {
                println!("[UI] Failed to create RenderTarget: {:?}", e);
            } else {
                let status_lock = overlay.current_status.lock().unwrap();
                let r = overlay.renderable.lock().unwrap();
                renderer.draw_frame(&status_lock, &*r);
            }
        }

        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        LRESULT(0)
    }

    fn on_size(&self, lparam: LPARAM) -> LRESULT {
        let width = (lparam.0 & 0xFFFF) as u32;
        let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
        let renderer = self.renderer.lock().unwrap();
        renderer.resize(width, height);
        LRESULT(0)
    }

    fn on_ncdestroy(hwnd: HWND) -> LRESULT {
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER_ID_FADE);
        }
        let overlay_ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *const OverlayWindow };
        if !overlay_ptr.is_null() {
            let _ = unsafe { Arc::from_raw(overlay_ptr) };
        }
        LRESULT(0)
    }

    fn on_timer(&self, hwnd: HWND, timer_id: usize) -> LRESULT {
        if timer_id == TIMER_ID_FADE {
            let _ = Self::update_position(self, hwnd);

            let phase = {
                let anim = self.animation.lock().unwrap();
                anim.get_phase()
            };

            if phase == AnimationPhase::Finished {
                println!("[UI] Animation finished, hiding window");
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
            WM_USER_UPDATE_POSITION => match Self::get_overlay(hwnd) {
                Some(overlay) => Self::update_position(overlay, hwnd),
                None => LRESULT(0),
            },
            WM_USER_SHOW_AND_FADE => Self::on_show_and_fade(hwnd),
            WM_TIMER => match Self::get_overlay(hwnd) {
                Some(overlay) => Self::on_timer(overlay, hwnd, wparam.0),
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
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
