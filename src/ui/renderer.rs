use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_numerics::Vector2;

use crate::ui::layout::LayoutManager;
use crate::ui::part::ImePart;

pub struct UiRenderer {
    pub d2d_factory: ID2D1Factory,
    pub render_target: Option<ID2D1HwndRenderTarget>,
    pub white_brush: Option<ID2D1SolidColorBrush>,
    pub highlight_brush: Option<ID2D1SolidColorBrush>,
    pub border_brush: Option<ID2D1SolidColorBrush>,
}

impl UiRenderer {
    pub fn new(d2d_factory: ID2D1Factory) -> Self {
        Self {
            d2d_factory,
            render_target: None,
            white_brush: None,
            highlight_brush: None,
            border_brush: None,
        }
    }

    pub fn ensure_target(&mut self, hwnd: HWND) -> Result<()> {
        if self.render_target.is_some() {
            return Ok(());
        }

        let mut rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect)? };
        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        println!(
            "[UI] Creating RenderTarget for hwnd: {:?}, size: {}x{}",
            hwnd, width, height
        );

        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 0.0,
            dpiY: 0.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U { width, height },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        let rt = unsafe { self.d2d_factory.CreateHwndRenderTarget(&props, &hwnd_props)? };
        self.create_brushes(&rt)?;
        self.render_target = Some(rt);
        println!("[UI] RenderTarget created for hwnd: {:?}", hwnd);
        Ok(())
    }

    pub fn create_brushes(&mut self, rt: &ID2D1HwndRenderTarget) -> Result<()> {
        self.white_brush = Some(unsafe {
            rt.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )?
        });
        self.highlight_brush = Some(unsafe {
            rt.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.26,
                    g: 0.26,
                    b: 0.26,
                    a: 0.2,
                },
                None,
            )?
        });
        self.border_brush = Some(unsafe {
            rt.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.1,
                },
                None,
            )?
        });
        Ok(())
    }

    pub fn draw_frame(&mut self, _status: &crate::ime::ImeStatus, parts: &[ImePart], height: f32) {
        let rt = match self.render_target.as_ref() {
            Some(rt) => rt,
            None => return,
        };
        let white = match self.white_brush.as_ref() {
            Some(b) => b,
            None => return,
        };
        let highlight = match self.highlight_brush.as_ref() {
            Some(b) => b,
            None => return,
        };
        let border = match self.border_brush.as_ref() {
            Some(b) => b,
            None => return,
        };

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));
        }

        let mut current_x = LayoutManager::PADDING_X;
        for (i, part) in parts.iter().enumerate() {
            let item_rect = LayoutManager::item_rect(current_x, part.width, height);

            if i == 0 {
                let rounded_rect = D2D1_ROUNDED_RECT {
                    rect: item_rect,
                    radiusX: 6.0,
                    radiusY: 6.0,
                };
                unsafe {
                    rt.FillRoundedRectangle(&rounded_rect, highlight);
                    rt.DrawRoundedRectangle(&rounded_rect, border, 0.5, None);
                }
            }

            println!("[UI] Drawing text part: {}, at x: {}", part.text, item_rect.left);
            unsafe {
                rt.DrawTextLayout(
                    Vector2 {
                        X: item_rect.left,
                        Y: item_rect.top,
                    },
                    &part.layout,
                    white,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }

            current_x += part.width + LayoutManager::GAP;
        }

        println!("[UI] EndDraw calling...");
        if let Err(e) = unsafe { rt.EndDraw(None, None) } {
            println!("[UI] EndDraw error: {:?}", e);
            self.discard_resources();
        } else {
            println!("[UI] EndDraw success.");
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(rt) = self.render_target.as_ref() {
            let _ = unsafe { rt.Resize(&D2D_SIZE_U { width, height }) };
        }
    }

    pub fn discard_resources(&mut self) {
        self.render_target = None;
        self.white_brush = None;
        self.highlight_brush = None;
        self.border_brush = None;
    }
}
