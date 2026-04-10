use crate::ui::parts::render::Renderable;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub struct PartBase {
    pub bg_color: Option<D2D1_COLOR_F>,
    pub color: Option<D2D1_COLOR_F>,
    pub border_color: Option<D2D1_COLOR_F>,
    pub radius: Option<f32>,
    pub childs: Vec<Box<dyn Renderable>>,
    pub pad_x: f32,
    pub pad_y: f32,
}

impl PartBase {
    pub fn new() -> Self {
        Self {
            bg_color: None,
            color: None,
            border_color: None,
            radius: None,
            childs: vec![],
            pad_x: 0.0,
            pad_y: 0.0,
        }
    }
}

pub trait Part {
    fn base(&self) -> &PartBase;
    fn content_width(&self) -> f32;
    fn content_height(&self) -> f32;
    fn padding_x(&self) -> f32 {
        self.base().pad_x
    }
    fn padding_y(&self) -> f32 {
        self.base().pad_y
    }
    fn bg_color(&self) -> Option<D2D1_COLOR_F> {
        self.base().bg_color
    }
    fn color(&self) -> Option<D2D1_COLOR_F> {
        self.base().color
    }
    fn border_color(&self) -> Option<D2D1_COLOR_F> {
        self.base().border_color
    }
    fn radius(&self) -> Option<f32> {
        self.base().radius
    }
    fn childs(&self) -> &[Box<dyn Renderable>] {
        self.base().childs.as_slice()
    }
    fn outer_width(&self) -> f32 {
        self.content_width() + self.padding_x() * 2.0
    }
    fn outer_height(&self) -> f32 {
        self.content_height() + self.padding_y() * 2.0
    }
}
