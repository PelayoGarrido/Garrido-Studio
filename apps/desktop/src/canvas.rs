use crate::brand::Theme;
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Painter, Stroke, StrokeKind};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}
pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        origin: Point { x, y },
        size: Size { width, height },
    }
}
pub fn inside(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x
        && p.x < r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y < r.origin.y + r.size.height
}
pub const ACCENT: [f64; 3] = crate::brand::CORAL;
pub const TEXT: [f64; 3] = [0.74, 0.76, 0.77];
pub const DIM: [f64; 3] = [0.43, 0.45, 0.46];
pub const EDGE: [f64; 3] = [0.12, 0.12, 0.12];
pub const ICON: [f64; 3] = [0.69, 0.74, 0.78];
pub const PANEL: [f64; 3] = [0.2, 0.2, 0.2];
pub fn color(rgb: [f64; 3]) -> Color32 {
    Color32::from_rgb(
        (rgb[0].clamp(0., 1.) * 255.).round() as u8,
        (rgb[1].clamp(0., 1.) * 255.).round() as u8,
        (rgb[2].clamp(0., 1.) * 255.).round() as u8,
    )
}

pub struct Canvas {
    painter: Painter,
    origin: egui::Pos2,
    pub scale: f64,
    theme: Theme,
}
impl Canvas {
    pub fn new(painter: Painter, origin: egui::Pos2, scale: f64, theme: Theme) -> Self {
        Self {
            painter,
            origin,
            scale,
            theme,
        }
    }
    pub fn point(&self, p: Point) -> egui::Pos2 {
        self.origin + egui::vec2((p.x * self.scale) as f32, (p.y * self.scale) as f32)
    }
    pub fn rect(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_size(
            self.point(r.origin),
            egui::vec2(
                (r.size.width * self.scale) as f32,
                (r.size.height * self.scale) as f32,
            ),
        )
    }
    pub fn design_point(&self, p: egui::Pos2) -> Point {
        let p = p - self.origin;
        Point {
            x: p.x as f64 / self.scale,
            y: p.y as f64 / self.scale,
        }
    }
    pub fn fill(&self, r: Rect, tint: [f64; 3]) {
        self.round(r, 0., tint)
    }
    pub fn round(&self, r: Rect, radius: f64, tint: [f64; 3]) {
        if r.size.width > 0. && r.size.height > 0. {
            self.painter.rect_filled(
                self.rect(r),
                egui::CornerRadius::same((radius * self.scale).round().clamp(0., 255.) as u8),
                color(if r.size.width <= 2. || r.size.height <= 2. {
                    self.theme.stroke(tint)
                } else {
                    self.theme.surface(tint)
                }),
            );
        }
    }
    pub fn outline(&self, r: Rect, radius: f64, width: f64, tint: [f64; 3]) {
        self.painter.rect_stroke(
            self.rect(r),
            egui::CornerRadius::same((radius * self.scale).round().clamp(0., 255.) as u8),
            Stroke::new((width * self.scale) as f32, color(self.theme.stroke(tint))),
            StrokeKind::Inside,
        );
    }
    pub fn line(&self, a: Point, b: Point, width: f64, tint: [f64; 3]) {
        self.painter.line_segment(
            [self.point(a), self.point(b)],
            Stroke::new((width * self.scale) as f32, color(self.theme.stroke(tint))),
        );
    }
    pub fn circle(&self, x: f64, y: f64, radius: f64, tint: [f64; 3]) {
        self.painter.circle_filled(
            self.point(Point { x, y }),
            (radius * self.scale) as f32,
            color(self.theme.surface(tint)),
        );
    }
    pub fn label(&self, text: &str, x: f64, y: f64, size: f64, tint: [f64; 3], _bold: bool) {
        self.painter.text(
            self.point(Point { x, y }),
            Align2::LEFT_TOP,
            text,
            FontId::new((size * self.scale) as f32, FontFamily::Proportional),
            color(self.theme.ink(tint)),
        );
    }
    pub fn text(&self, text: &str, x: f64, y: f64, size: f64) {
        self.label(text, x, y, size.max(14.), TEXT, false)
    }
    pub fn icon(&self, name: &str, x: f64, y: f64, size: f64, tint: [f64; 3]) {
        self.painter.text(
            self.point(Point { x, y }),
            Align2::LEFT_TOP,
            super::icon_codes::glyph(name),
            FontId::new(
                (size * self.scale) as f32,
                FontFamily::Name("Tabler".into()),
            ),
            color(self.theme.ink(tint)),
        );
    }
    pub fn border(&self, r: Rect) {
        self.round(r, 11., PANEL);
        self.outline(r, 11., 1., EDGE);
    }
    pub fn image(&self, texture: egui::TextureId, r: Rect) {
        self.painter.add(
            egui::epaint::RectShape::filled(
                self.rect(r),
                egui::CornerRadius::same((10. * self.scale).round() as u8),
                Color32::WHITE,
            )
            .with_texture(
                texture,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            ),
        );
    }
    pub fn thumb(&self, r: Rect, radius: f64) {
        self.painter.rect_filled(
            self.rect(r),
            egui::CornerRadius::same((radius * self.scale).round() as u8),
            color(self.theme.thumb()),
        );
    }
    pub fn piano_key(&self, r: Rect) {
        self.painter.rect_filled(
            self.rect(r),
            egui::CornerRadius::same(3),
            color(crate::brand::PLUM),
        );
    }
    pub fn clipped(&self, r: Rect) -> Self {
        Self::new(
            self.painter.with_clip_rect(self.rect(r)),
            self.origin,
            self.scale,
            self.theme,
        )
    }
}

impl Canvas {
    pub fn knob(&self, x: f64, y: f64, radius: f64, value: f64, active: bool) {
        self.circle(x, y, radius + 1., [0.16, 0.16, 0.16]);
        self.circle(x, y, radius - 2., [0.25, 0.25, 0.25]);
        for i in 0..32 {
            let a = (145. + i as f64 * 8.).to_radians();
            let b = (145. + (i + 1) as f64 * 8.).to_radians();
            self.line(
                Point {
                    x: x + radius * a.cos(),
                    y: y + radius * a.sin(),
                },
                Point {
                    x: x + radius * b.cos(),
                    y: y + radius * b.sin(),
                },
                1.5,
                if active && i as f64 / 32. < value {
                    ACCENT
                } else {
                    [0.29, 0.29, 0.29]
                },
            );
        }
        let angle = (145. + value.clamp(0., 1.) * 256.).to_radians();
        self.line(
            Point { x, y },
            Point {
                x: x + (radius - 5.) * angle.cos(),
                y: y + (radius - 5.) * angle.sin(),
            },
            1.4,
            TEXT,
        );
    }
    pub fn segments(&self, text: &str, x: f64, y: f64, width: f64, height: f64) {
        let mut xx = x;
        let masks = [
            0b1111110, 0b0110000, 0b1101101, 0b1111001, 0b0110011, 0b1011011, 0b1011111, 0b1110000,
            0b1111111, 0b1111011,
        ];
        for c in text.chars() {
            if c == ':' {
                self.circle(xx + 2., y + height * 0.3, 1., ACCENT);
                self.circle(xx + 2., y + height * 0.73, 1., ACCENT);
                xx += 5.;
                continue;
            }
            let Some(n) = c.to_digit(10) else {
                xx += width;
                continue;
            };
            let mid = height * 0.5;
            for (bit, (a, b)) in [
                ((2., 0.), (width - 2., 0.)),
                ((width, 2.), (width, mid - 2.)),
                ((width, mid + 2.), (width, height - 2.)),
                ((width - 2., height), (2., height)),
                ((0., height - 2.), (0., mid + 2.)),
                ((0., mid - 2.), (0., 2.)),
                ((2., mid), (width - 2., mid)),
            ]
            .into_iter()
            .enumerate()
            {
                if masks[n as usize] & (1 << (6 - bit)) != 0 {
                    self.line(
                        Point {
                            x: xx + a.0,
                            y: y + a.1,
                        },
                        Point {
                            x: xx + b.0,
                            y: y + b.1,
                        },
                        1.4,
                        ACCENT,
                    );
                }
            }
            xx += width + 3.;
        }
    }
}
