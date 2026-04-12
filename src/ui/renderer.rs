use crate::ui::parts::Renderable;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct UiRenderer {
    pub d2d_factory: ID2D1Factory,
    pub render_target: Option<ID2D1HwndRenderTarget>,
}

impl UiRenderer {
    pub fn new(d2d_factory: ID2D1Factory) -> Self {
        Self { d2d_factory, render_target: None }
    }

    pub fn ensure_target(&mut self, hwnd: HWND) -> Result<()> {
        if self.render_target.is_some() {
            return Ok(());
        }

        let mut rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect)? };
        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        log::info!("Creating RenderTarget for hwnd: {:?}, size: {}x{}", hwnd, width, height);

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
        self.render_target = Some(rt);
        log::info!("RenderTarget created for hwnd: {:?}", hwnd);
        Ok(())
    }

    pub fn draw_frame(&mut self, _status: &crate::ime::ImeStatus, row: &dyn Renderable) {
        let rt = match self.render_target.as_ref() {
            Some(rt) => rt,
            None => return,
        };

        unsafe {
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        }

        row.render(rt, 0.0, 0.0);

        if let Err(e) = unsafe { rt.EndDraw(None, None) } {
            log::error!("EndDraw error: {:?}", e);
            self.discard_resources();
        } else {
            log::debug!("EndDraw");
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(rt) = self.render_target.as_ref() {
            let _ = unsafe { rt.Resize(&D2D_SIZE_U { width, height }) };
        }
    }

    pub fn discard_resources(&mut self) {
        self.render_target = None;
    }
}
