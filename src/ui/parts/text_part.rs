use super::part_trait::Part;
use super::render::Renderable;
use crate::ui::parts::{Padding, PartBase};
use std::sync::OnceLock;
use windows::core::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows_numerics::Vector2;

struct TextFormatState {
    factory: IDWriteFactory,
    format: IDWriteTextFormat,
}

static TEXT_FORMAT: OnceLock<TextFormatState> = OnceLock::new();

#[allow(dead_code)]
pub struct TextPart {
    pub base: PartBase,
    pub text: String,
    pub layout: IDWriteTextLayout,
    pub text_width: f32,
    pub text_height: f32,
}

impl TextPart {
    pub fn init(font_name: &str, font_size: f32) {
        TEXT_FORMAT.get_or_init(|| unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .expect("Failed to create DWriteFactory");
            let font_name_wide: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();
            let format = factory
                .CreateTextFormat(
                    PCWSTR(font_name_wide.as_ptr()),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_size,
                    w!("en-US"),
                )
                .expect("Failed to create IDWriteTextFormat");
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER).ok();
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER).ok();
            TextFormatState { factory, format }
        });
    }

    fn state() -> &'static TextFormatState {
        TEXT_FORMAT.get().expect("TextPart::init must be called before use")
    }

    fn measure_text(wide: &Vec<u16>) -> Result<(f32, f32)> {
        let s = Self::state();
        let measure_layout = unsafe { s.factory.CreateTextLayout(wide, &s.format, f32::MAX, f32::MAX)? };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { measure_layout.GetMetrics(&mut metrics)? };
        Ok((metrics.width, metrics.height))
    }

    pub fn with_color(
        text: &str,
        color: Common::D2D1_COLOR_F,
        padding: Option<Padding>
    ) -> Result<Box<dyn Renderable>> {
        let s = Self::state();
        let wide: Vec<u16> = text.encode_utf16().collect();
        let (text_width, text_height) = Self::measure_text(&wide)?;
        let layout = unsafe { s.factory.CreateTextLayout(&wide, &s.format, text_width, text_height)? };
        Ok(Box::new(Self {
            base: PartBase {
                color: Some(color),
                padding: padding.unwrap_or_default(),
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
