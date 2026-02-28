use crate::ui::part::ImePart;
use windows::Win32::Graphics::Direct2D::Common::*;

pub struct LayoutManager;

impl LayoutManager {
    pub const PADDING_X: f32 = 8.0;
    pub const GAP: f32 = 4.0;
    pub const PADDING_Y: f32 = 4.0;

    pub fn total_width(parts: &[ImePart]) -> f32 {
        let mut w = Self::PADDING_X;
        for part in parts {
            w += part.width + Self::GAP;
        }
        w + Self::GAP // trailing padding
    }

    pub fn item_rect(current_x: f32, item_width: f32, height: f32) -> D2D_RECT_F {
        D2D_RECT_F {
            left: current_x,
            top: Self::PADDING_Y,
            right: current_x + item_width,
            bottom: height - Self::PADDING_Y,
        }
    }
}
