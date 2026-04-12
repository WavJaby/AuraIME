use crate::ime::ImeStatus;
use crate::monitor::caret;
use crate::ui::accent::setup_modern_look;
use crate::ui::animation::{AnimationPhase, AnimationState};
use crate::ui::parts::{Container, Padding, Part, Renderable, TextPart};
use crate::ui::renderer::UiRenderer;
use crate::ui::window_helper::{
    get_monitor_work_area, get_window_dpi_scale, init_window, post_window_message, set_window_pos_topmost,
    set_window_size,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::Ime::IME_CMODE_NATIVE;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_USER_SHOW_AND_FADE: u32 = WM_USER + 2;
const WM_USER_TICK: u32 = WM_USER + 3;

pub struct OverlayWindow {
    pub hwnd: HWND,
    pub renderer: Mutex<UiRenderer>,
    pub renderable: Mutex<Container>,
    pub current_status: Mutex<ImeStatus>,
    pub animation: Mutex<AnimationState>,
    pub last_state: Mutex<LastWindowState>,
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

impl OverlayWindow {
    pub fn new() -> Result<Arc<Self>> {
        let window_class = w!("AuraIME_Overlay");
        let window_name = w!("AuraIME");

        let d2d_factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        TextPart::init("Segoe UI Variable Text", 16.0);

        let overlay = Arc::new(Self {
            hwnd: HWND::default(),
            renderer: Mutex::new(UiRenderer::new(d2d_factory)),
            renderable: Mutex::new(Container::empty()),
            current_status: Mutex::new(ImeStatus::default()),
            animation: Mutex::new(AnimationState::new()),
            last_state: Mutex::new(LastWindowState::new()),
            vsync_running: Arc::new(AtomicBool::new(false)),
        });

        let hwnd = match init_window(Some(Self::wnd_proc), Arc::as_ptr(&overlay), window_class, window_name) {
            Ok(hwnd) => hwnd,
            Err(e) => {
                log::error!("Failed to create window: {:?}", e);
                return Err(e);
            }
        };

        match setup_modern_look(hwnd) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to setup modern look: {:?}", e);
                return Err(e);
            }
        };

        Ok(overlay)
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
            post_window_message(self.hwnd, WM_USER_SHOW_AND_FADE)?;
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
                post_window_message(self.hwnd, WM_USER_SHOW_AND_FADE)?;
            }
        }

        self.resize_to_content()?;
        let _ = unsafe { InvalidateRect(Some(self.hwnd), None, true) };

        Ok(())
    }

    pub fn resize_to_content(&self) -> Result<()> {
        let (total_width, total_height) = {
            let elements = self.renderable.lock().unwrap();
            (elements.outer_width().ceil() as i32, elements.outer_height().ceil() as i32)
        };

        set_window_size(self.hwnd, total_width, total_height)?;
        Ok(())
    }

    fn update_position(&self) -> Result<()> {
        let caret_info = caret::get_caret_rect(self.hwnd);

        // Calculate total size of elements
        let (total_width, total_height) = {
            let elements = self.renderable.lock().unwrap();
            (elements.outer_width().ceil() as i32, elements.outer_height().ceil() as i32)
        };

        // Animation time
        let t = self.animation.lock().unwrap().get_time();

        let mut last = self.last_state.lock().unwrap();
        if let Some(info) = caret_info {
            if info.rect != last.caret_rect {
                last.caret_rect = info.rect;
            }
        }

        // Update window transparency, fades in/out
        let alpha = (t * 255.0) as u8;
        if last.alpha != alpha {
            let _ = unsafe { SetLayeredWindowAttributes(self.hwnd, COLORREF(0), alpha, LWA_ALPHA) };
            last.alpha = alpha;
        }

        // Slide in/out animation
        let scale = get_window_dpi_scale(self.hwnd)?;
        let total_offset_y = 20f32;
        let padding_x = 8f32;
        let padding_y = 8f32;
        let offset_y = (((total_offset_y + padding_y) - (t * total_offset_y)) * scale) as i32;

        // Check if placing below caret would overflow the screen
        let screen = get_monitor_work_area(&last.caret_rect)?;

        // Clamp x to screen boundaries
        let x_max = screen.right - total_width;
        let x = (last.caret_rect.left + padding_x as i32).clamp(screen.left, x_max);

        // Clamp y with offset, ensuring no overflow
        let y_below_total = last.caret_rect.bottom + total_height + total_offset_y as i32;
        let y_below = last.caret_rect.bottom + offset_y;
        let above_fits = last.caret_rect.top - total_height >= screen.top;
        let y = if y_below_total > screen.bottom && above_fits {
            last.caret_rect.top - total_height - offset_y
        } else {
            y_below
        };

        if last.x != x || last.y != y {
            set_window_pos_topmost(self.hwnd, x, y)?;
            last.x = x;
            last.y = y;
        }

        Ok(())
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

    fn start_vsync_thread(&self) {
        // Only start a new vsync thread if isn't running
        if !self.vsync_running.swap(true, Ordering::SeqCst) {
            let running = self.vsync_running.clone();
            let hwnd_raw = self.hwnd.0 as isize;
            std::thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
                while running.load(Ordering::SeqCst) {
                    let _ = unsafe { DwmFlush() };
                    post_window_message(hwnd, WM_USER_TICK)
                        .unwrap_or_else(|e| log::error!("Failed to post WM_USER_TICK message: {:?}", e));
                }
            });
        }
    }

    fn on_show_and_fade(&self) -> LRESULT {
        let _ = unsafe { SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 0, LWA_ALPHA) };
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };

        self.start_vsync_thread();

        LRESULT(0)
    }

    fn on_paint(&self) -> LRESULT {
        log::debug!("WM_PAINT");

        let mut ps = PAINTSTRUCT::default();
        let _hdc = unsafe { BeginPaint(self.hwnd, &mut ps) };

        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(self.hwnd, &mut rect) };
        {
            let mut renderer = self.renderer.lock().unwrap();
            if let Err(e) = renderer.ensure_target(self.hwnd) {
                log::error!("Failed to create RenderTarget: {:?}", e);
            } else {
                let status_lock = self.current_status.lock().unwrap();
                let r = self.renderable.lock().unwrap();
                renderer.draw_frame(&status_lock, &*r);
            }
        }

        let _ = unsafe { EndPaint(self.hwnd, &ps) };
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

    fn on_tick(&self) -> LRESULT {
        let _ = self.update_position();

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
                let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
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
            WM_USER_SHOW_AND_FADE => match Self::get_overlay(hwnd) {
                Some(overlay) => overlay.on_show_and_fade(),
                None => LRESULT(0),
            },
            WM_USER_TICK => match Self::get_overlay(hwnd) {
                Some(overlay) => overlay.on_tick(),
                None => LRESULT(0),
            },
            WM_PAINT => match Self::get_overlay(hwnd) {
                Some(overlay) => overlay.on_paint(),
                None => {
                    let mut ps = PAINTSTRUCT::default();
                    unsafe { BeginPaint(hwnd, &mut ps) };
                    let _ = unsafe { EndPaint(hwnd, &ps) };
                    LRESULT(0)
                }
            },
            WM_SIZE => match Self::get_overlay(hwnd) {
                Some(overlay) => overlay.on_size(lparam),
                None => LRESULT(0),
            },
            WM_NCDESTROY => Self::on_ncdestroy(hwnd),
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
