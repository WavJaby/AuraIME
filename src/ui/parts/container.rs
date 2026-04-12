use super::part_trait::{Border, Padding, Part, PartBase};
use super::render::Renderable;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;

pub struct Container {
    pub base: PartBase,
    pub gap: f32,
}

impl Container {
    pub fn empty() -> Self {
        Self {
            base: PartBase::new(),
            gap: 0.0,
        }
    }

    pub fn new(childs: Vec<Box<dyn Renderable>>, pad_x: f32, pad_y: f32, gap: f32) -> Self {
        Self {
            base: PartBase {
                bg_color: None,
                color: None,
                border: Border::default(),
                childs,
                padding: Padding::symmetric(pad_x, pad_y),
            },
            gap,
        }
    }

    pub fn new_with_color(
        childs: Vec<Box<dyn Renderable>>,
        pad_x: f32,
        pad_y: f32,
        radius: f32,
        gap: f32,
        bg_color: D2D1_COLOR_F,
        border_color: D2D1_COLOR_F,
    ) -> Box<dyn Renderable> {
        Box::new(Container {
            base: PartBase {
                bg_color: Some(bg_color),
                color: None,
                border: Border::new(border_color, radius),
                childs,
                padding: Padding::symmetric(pad_x, pad_y),
            },
            gap,
        })
    }
}

impl Part for Container {
    fn base(&self) -> &PartBase {
        &self.base
    }
    fn content_width(&self) -> f32 {
        self.base.childs.iter().map(|c| c.outer_width()).sum::<f32>()
            + self.gap * self.base.childs.len().saturating_sub(1) as f32
    }
    fn content_height(&self) -> f32 {
        self.base
            .childs
            .iter()
            .map(|c| c.outer_height())
            .fold(0.0_f32, f32::max)
    }
}

impl Renderable for Container {
    fn render(&self, rt: &ID2D1HwndRenderTarget, origin_x: f32, origin_y: f32) {
        let bg_rect = D2D_RECT_F {
            left: origin_x,
            top: origin_y,
            right: origin_x + self.outer_width(),
            bottom: origin_y + self.outer_height(),
        };
        let radius = self.base.border.radius;
        let rounded = D2D1_ROUNDED_RECT {
            rect: bg_rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            if let Some(brush) = self.base.bg_color.and_then(|c| rt.CreateSolidColorBrush(&c, None).ok()) {
                rt.FillRoundedRectangle(&rounded, &brush);
            }
            if let Some(brush) = self
                .base
                .border
                .color
                .and_then(|c| rt.CreateSolidColorBrush(&c, None).ok())
            {
                rt.DrawRoundedRectangle(&rounded, &brush, 0.5, None);
            }
        }

        let container_height = self.content_height();

        let mut x_offset = self.base.padding.left;
        for child in &self.base.childs {
            let child_height = child.outer_height();
            let y_offset = (container_height - child_height) / 2.0;

            child.render(rt, origin_x + x_offset, origin_y + y_offset + self.base.padding.top);
            x_offset += child.outer_width() + self.gap;
        }
    }
}
