use super::part_trait::Part;
use super::render::Renderable;
use crate::ui::parts::PartBase;
use windows::core::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows_numerics::Vector2;

pub struct TextPart {
    pub base: PartBase,
    pub text: String,
    pub layout: IDWriteTextLayout,
    pub text_width: f32,
    pub text_height: f32,
}

impl TextPart {
    pub fn measure_text(
        wide: &Vec<u16>,
        factory: &IDWriteFactory,
        text_format: &IDWriteTextFormat,
    ) -> Result<(f32, f32)> {
        let measure_layout = unsafe { factory.CreateTextLayout(wide, text_format, f32::MAX, f32::MAX)? };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { measure_layout.GetMetrics(&mut metrics)? };
        Ok((metrics.width, metrics.height))
    }

    pub fn with_color(
        text: &str,
        dwrite_factory: &IDWriteFactory,
        text_format: &IDWriteTextFormat,
        color: Common::D2D1_COLOR_F,
    ) -> Result<Box<dyn Renderable>> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let (text_width, text_height) = Self::measure_text(&wide, dwrite_factory, text_format)?;
        let layout = unsafe { dwrite_factory.CreateTextLayout(&wide, text_format, text_width, text_height)? };
        Ok(Box::new(Self {
            base: PartBase {
                color: Some(color),
                ..PartBase::new()
            },
            text: text.to_string(),
            layout,
            text_width,
            text_height,
        }))
    }
}

impl Part for TextPart {
    fn base(&self) -> &PartBase {
        &self.base
    }
    fn content_width(&self) -> f32 {
        self.text_width
    }
    fn content_height(&self) -> f32 {
        self.text_height
    }
}

impl Renderable for TextPart {
    fn render(&self, rt: &ID2D1HwndRenderTarget, origin_x: f32, origin_y: f32) {
        unsafe {
            if let Some(b) = self.base.color.and_then(|c| rt.CreateSolidColorBrush(&c, None).ok()) {
                rt.DrawTextLayout(
                    Vector2 {
                        X: origin_x,
                        Y: origin_y,
                    },
                    &self.layout,
                    &b,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
        }
    }
}
