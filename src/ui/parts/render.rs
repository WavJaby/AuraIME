use super::part_trait::Part;
use crate::ui::parts::PartBase;
use windows::Win32::Graphics::Direct2D::*;

pub trait Renderable: Part {
    fn render(&self, rt: &ID2D1HwndRenderTarget, origin_x: f32, origin_y: f32);
}

impl Part for Box<dyn Renderable> {
    fn base(&self) -> &PartBase {
        (**self).base()
    }

    fn content_width(&self) -> f32 {
        (**self).content_width()
    }

    fn content_height(&self) -> f32 {
        (**self).content_height()
    }

    fn padding_x(&self) -> f32 {
        (**self).padding_x()
    }

    fn padding_y(&self) -> f32 {
        (**self).padding_y()
    }
}

impl Renderable for Box<dyn Renderable> {
    fn render(&self, rt: &ID2D1HwndRenderTarget, origin_x: f32, origin_y: f32) {
        (**self).render(rt, origin_x, origin_y);
    }
}
