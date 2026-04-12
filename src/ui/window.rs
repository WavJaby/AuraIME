use crate::ime::ImeStatus;
use crate::monitor::caret;
use crate::ui::accent;
use crate::ui::animation::{AnimationPhase, AnimationState};
use crate::ui::parts::{Container, Padding, Part, Renderable, TextPart};
use crate::ui::renderer::UiRenderer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::Ime::IME_CMODE_NATIVE;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_USER_UPDATE_POSITION: u32 = WM_USER + 1;
const WM_USER_SHOW_AND_FADE: u32 = WM_USER + 2;
const WM_USER_TICK: u32 = WM_USER + 3;

#[derive(Clone)]
pub struct OverlayWindow {
    pub hwnd: HWND,
    pub renderer: Arc<Mutex<UiRenderer>>,
    pub renderable: Arc<Mutex<Container>>,
    pub current_status: Arc<Mutex<ImeStatus>>,
    pub animation: Arc<Mutex<AnimationState>>,
    pub last_state: Arc<Mutex<LastWindowState>>,
    pub vsync_running: Arc<AtomicBool>,
}

#[allow(dead_code)]
pub struct LastWindowState {
    pub alpha: u8,
    pub x: i32,
    pub y: i32,
    pub caret_rect: RECT,
    pub hwnd: HWND,
}

impl LastWindowState {
    pub fn new() -> Self {
        Self { alpha: 0, x: 0, y: 0, caret_rect: RECT::default(), hwnd: HWND::default() }
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
            let window_name = w!("AuraIME");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: instance.into(),
                lpszClassName: window_class,
                hbrBackground: HBRUSH::default(),
                ..Default::default()
            };

            RegisterClassW(&wc);

            let d2d_factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

            TextPart::init("Segoe UI Variable Text", 16.0);

            let overlay = Arc::new(Self {
                hwnd: HWND::default(),
                renderer: Arc::new(Mutex::new(UiRenderer::new(d2d_factory))),
                renderable: Arc::new(Mutex::new(Container::empty())),
                current_status: Arc::new(Mutex::new(ImeStatus::default())),
                animation: Arc::new(Mutex::new(AnimationState::new())),
                last_state: Arc::new(Mutex::new(LastWindowState::new())),
                vsync_running: Arc::new(AtomicBool::new(false)),
            });

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                window_class,
                window_name,
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                Some(Arc::as_ptr(&overlay) as *const _),
            )?;

            Self::setup_modern_look(hwnd);

            Ok(overlay)
        }
    }

    pub fn setup_modern_look(hwnd: HWND) {
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
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

    #[allow(dead_code)]
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
            log::info!("Caret moved, showing window.");
            let _ = unsafe { PostMessageW(Some(self.hwnd), WM_USER_SHOW_AND_FADE, WPARAM(0), LPARAM(0)) };
        }
        drop(anim_lock);

        Ok(())
    }

    pub fn update_status(&self, status: ImeStatus) -> Result<()> {
        if !status.has_caret {
            return Ok(());
        }

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

        let full_width = if status.full_width { Some("●") } else { None };

        let text_color = D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut childs: Vec<Box<dyn Renderable>> = Vec::new();
        // Add base name
        let base = TextPart::with_color(&status.display_name, text_color, None)?;
        childs.push(Container::new_with_color(
            vec![base],
            8.0,
            4.0,
            4.0,
            8.0,
            D2D1_COLOR_F { r: 0.26, g: 0.26, b: 0.26, a: 0.2 },
            D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 0.1 },
        ));

        // Add ime status indicator
        if let Some(ind) = indicator {
            childs.push(TextPart::with_color(ind, text_color, None)?);
        }

        // Add full width indicator
        if let Some(full_width) = full_width {
            childs.push(TextPart::with_color(full_width, text_color, Padding::bottom(4.0))?);
        }

        *self.renderable.lock().unwrap() = Container::new(childs, 8.0, 8.0, 8.0);

        log::info!("Status updated to: {} {}", status.display_name, indicator.unwrap_or(""));
        {
            let mut anim_lock = self.animation.lock().unwrap();
            let skip_fade_in = anim_lock.on_activity();
            if !skip_fade_in {
                let _ = unsafe { PostMessageW(Some(self.hwnd), WM_USER_SHOW_AND_FADE, WPARAM(0), LPARAM(0)) };
            }
        }

        unsafe {
            self.resize_to_content()?;
            let _ = InvalidateRect(Some(self.hwnd), None, true);
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
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
            )
        };
        Ok(())
    }

    fn update_position(&self, hwnd: HWND) -> LRESULT {
        let caret_rect = caret::get_caret_rect(hwnd);

        let r = self.renderable.lock().unwrap();
        let total_width = r.outer_width();
        let total_height = r.outer_height();
        drop(r);

        let mut last = self.last_state.lock().unwrap();
        if let Some(rect) = caret_rect {
            if rect != last.caret_rect {
                last.caret_rect = rect;
            }
        }

        let t = {
            let anim = self.animation.lock().unwrap();
            anim.get_time()
        };

        unsafe {
            // Update window transparency, fades in/out
            let alpha = (t * 255.0) as u8;
            if last.alpha != alpha {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                last.alpha = alpha;
            }

            // Slide in/out animation
            if last.caret_rect.left != 0 || last.caret_rect.top != 0 {
                let dpi = GetDpiForWindow(hwnd);
                let scale = dpi as f32 / 96.0;
                let total_offset_y = 20f32;
                let padding_x = 8f32;
                let padding_y = 8f32;
                let offset_y = (((total_offset_y + padding_y) - (t * total_offset_y)) * scale) as i32;

                // Check if placing below caret would overflow the screen
                let monitor = MonitorFromRect(&last.caret_rect, MONITOR_DEFAULTTONEAREST);
                let mut monitor_info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(monitor, &mut monitor_info);
                let screen_left = monitor_info.rcWork.left;
                let screen_right = monitor_info.rcWork.right;
                let screen_bottom = monitor_info.rcWork.bottom;

                // Clamp x to screen boundaries
                let x_max = screen_right - total_width.ceil() as i32;
                let x = (last.caret_rect.left + padding_x as i32).clamp(screen_left, x_max);

                // Clamp y with offset, ensuring no overflow
                let y_below_total = last.caret_rect.bottom + (total_height + total_offset_y).ceil() as i32;
                let y_below = last.caret_rect.bottom + offset_y;
                let above_fits = last.caret_rect.top - total_height.ceil() as i32 >= monitor_info.rcWork.top;
                let y = if y_below_total > screen_bottom && above_fits {
                    last.caret_rect.top - total_height.ceil() as i32 - offset_y
                } else {
                    y_below
                };

                if last.x != x || last.y != y {
                    SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE)
                        .expect("SetWindowPos failed");
                    last.x = x;
                    last.y = y;
                }
            }
        }
        drop(last);
        LRESULT(0)
    }

    fn on_create(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        log::debug!("WM_CREATE");
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        let overlay_ptr = unsafe { (*create_struct).lpCreateParams as *mut OverlayWindow };

        unsafe { (*overlay_ptr).hwnd = hwnd };
        let overlay = unsafe { Arc::from_raw(overlay_ptr) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::into_raw(overlay) as isize) };

        LRESULT(0)
    }

    fn start_vsync_thread(overlay: &OverlayWindow, hwnd: HWND) {
        // Only start a new vsync thread if isn't running
        if !overlay.vsync_running.swap(true, Ordering::SeqCst) {
            let running = overlay.vsync_running.clone();
            let hwnd_raw = hwnd.0 as isize;
            std::thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
                while running.load(Ordering::SeqCst) {
                    unsafe {
                        let _ = DwmFlush();
                        let _ = PostMessageW(Some(hwnd), WM_USER_TICK, WPARAM(0), LPARAM(0));
                    }
                }
            });
        }
    }

    fn on_show_and_fade(hwnd: HWND) -> LRESULT {
        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }

        if let Some(overlay) = Self::get_overlay(hwnd) {
            Self::start_vsync_thread(overlay, hwnd);
        }

        LRESULT(0)
    }

    fn on_paint(hwnd: HWND, overlay: &OverlayWindow) -> LRESULT {
        log::debug!("WM_PAINT");

        let mut ps = PAINTSTRUCT::default();
        let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };
        {
            let mut renderer = overlay.renderer.lock().unwrap();
            if let Err(e) = renderer.ensure_target(hwnd) {
                log::error!("Failed to create RenderTarget: {:?}", e);
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
        let overlay_ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *const OverlayWindow };
        if !overlay_ptr.is_null() {
            let overlay = unsafe { Arc::from_raw(overlay_ptr) };
            overlay.vsync_running.store(false, Ordering::SeqCst);
        }
        LRESULT(0)
    }

    fn on_tick(&self, hwnd: HWND) -> LRESULT {
        let _ = Self::update_position(self, hwnd);

        let phase = {
            let anim = self.animation.lock().unwrap();
            anim.get_phase()
        };

        if phase == AnimationPhase::Finished {
            // Use compare_exchange to ensure hide logic runs exactly once
            if self
                .vsync_running
                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                log::info!("Window hide");
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
            WM_USER_TICK => match Self::get_overlay(hwnd) {
                Some(overlay) => Self::on_tick(overlay, hwnd),
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
