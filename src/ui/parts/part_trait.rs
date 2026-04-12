use crate::ui::parts::render::Renderable;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Clone, Copy, Default)]
pub struct Border {
    pub color: Option<D2D1_COLOR_F>,
    pub radius: f32,
}

impl Border {
    pub fn new(color: D2D1_COLOR_F, radius: f32) -> Self {
        Self { color: Some(color), radius }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Padding {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[allow(dead_code)]
impl Padding {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn symmetric(x: f32, y: f32) -> Self {
        Self { left: x, top: y, right: x, bottom: y }
    }

    pub(crate) fn bottom(bottom: f32) -> Option<Padding> {
        Some(Self { left: 0.0, top: 0.0, right: 0.0, bottom })
    }
}

pub struct PartBase {
    pub childs: Vec<Box<dyn Renderable>>,
    pub bg_color: Option<D2D1_COLOR_F>,
    pub color: Option<D2D1_COLOR_F>,
    pub border: Border,
    pub padding: Padding,
}

impl PartBase {
    pub fn new() -> Self {
        Self {
            childs: vec![],
            bg_color: None,
            color: None,
            border: Border::default(),
            padding: Padding::default(),
        }
    }
}

#[allow(dead_code)]
pub trait Part {
    fn base(&self) -> &PartBase;
    fn content_width(&self) -> f32;
    fn content_height(&self) -> f32;
    fn padding(&self) -> Padding {
        self.base().padding
    }
    fn outer_width(&self) -> f32 {
        self.content_width() + self.padding().left + self.padding().right
    }
    fn outer_height(&self) -> f32 {
        self.content_height() + self.padding().top + self.padding().bottom
    }
}
