use windows::core::*;
use windows::Win32::Graphics::DirectWrite::*;

pub struct ImePart {
    pub text: String,
    pub layout: IDWriteTextLayout,
    pub width: f32,
}

impl ImePart {
    pub fn new(
        text: &str,
        dwrite_factory: &IDWriteFactory,
        text_format: &IDWriteTextFormat,
        max_height: f32,
    ) -> Result<Self> {
        let wide: Vec<u16> = text.encode_utf16().collect();

        // First pass – measure natural width
        let measure_layout = unsafe { dwrite_factory.CreateTextLayout(&wide, text_format, 1000.0, max_height)? };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { measure_layout.GetMetrics(&mut metrics)? };
        let item_width = metrics.width + 16.0;

        // Second pass – constrained layout used for drawing
        let layout = unsafe { dwrite_factory.CreateTextLayout(&wide, text_format, item_width, max_height - 8.0)? };

        Ok(Self {
            text: text.to_string(),
            layout,
            width: item_width,
        })
    }
}
