use eframe::egui;

pub(crate) const CORAL: [f64; 3] = [1., 149. / 255., 108. / 255.];
pub(crate) const ROSE: [f64; 3] = [242. / 255., 109. / 255., 178. / 255.];
pub(crate) const VIOLET: [f64; 3] = [166. / 255., 92. / 255., 236. / 255.];
pub(crate) const PEARL: [f64; 3] = [252. / 255., 250. / 255., 1.];
pub(crate) const PLUM: [f64; 3] = [24. / 255., 21. / 255., 30. / 255.];
pub(crate) const LOGO: &[u8; 65536] = include_bytes!("../../../assets/brand/logo-128.rgba");

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Theme {
    #[default]
    Dark,
    Light,
}
fn neutral(rgb: [f64; 3]) -> bool {
    let low = rgb.into_iter().fold(f64::INFINITY, f64::min);
    let high = rgb.into_iter().fold(f64::NEG_INFINITY, f64::max);
    high - low < 0.095
}
impl Theme {
    pub(crate) fn surface(self, rgb: [f64; 3]) -> [f64; 3] {
        if rgb == PLUM || rgb == CORAL || rgb == ROSE || rgb == VIOLET {
            return rgb;
        }
        let value = rgb.iter().sum::<f64>() / 3.;
        if neutral(rgb) {
            if value > 0.85 {
                return PEARL;
            }
            match self {
                Self::Dark => [
                    value * 0.55 + 0.014,
                    value * 0.48 + 0.012,
                    value * 0.63 + 0.026,
                ],
                Self::Light => [
                    (0.91 + value * 0.3).min(1.),
                    (0.9 + value * 0.32).min(1.),
                    (0.94 + value * 0.2).min(1.),
                ],
            }
        } else if rgb[0] >= rgb[1] && rgb[1] > rgb[2] * 1.2 {
            match self {
                Self::Dark => [0.20 + value * 0.45, 0.12 + value * 0.22, 0.19 + value * 0.3],
                Self::Light => [1., 0.83 + value * 0.08, 0.89 + value * 0.08],
            }
        } else {
            rgb
        }
    }
    pub(crate) fn ink(self, rgb: [f64; 3]) -> [f64; 3] {
        if rgb == PLUM {
            return PLUM;
        }
        if rgb == CORAL || rgb == ROSE || rgb == VIOLET {
            return if self == Self::Light {
                [0.48, 0.16, 0.38]
            } else {
                rgb
            };
        }
        if neutral(rgb) {
            let value = rgb.iter().sum::<f64>() / 3.;
            match self {
                Self::Dark => [
                    0.32 + value * 0.72,
                    0.29 + value * 0.72,
                    0.38 + value * 0.65,
                ]
                .map(|v| v.min(1.)),
                Self::Light => [
                    0.13 + (1. - value) * 0.24,
                    0.105 + (1. - value) * 0.24,
                    0.17 + (1. - value) * 0.25,
                ],
            }
        } else if rgb[0] >= rgb[1] && rgb[1] > rgb[2] * 1.2 {
            [0.35, 0.13, 0.26]
        } else {
            rgb
        }
    }
    pub(crate) fn stroke(self, rgb: [f64; 3]) -> [f64; 3] {
        if neutral(rgb) && rgb.iter().sum::<f64>() / 3. >= 0.4 {
            return self.ink(rgb);
        }
        if self == Self::Light && (rgb == CORAL || rgb == ROSE || rgb == VIOLET) {
            return [0.51, 0.25, 0.75];
        }
        if self == Self::Light && neutral(rgb) {
            let value = rgb.iter().sum::<f64>() / 3.;
            return [0.76 + value * 0.2, 0.72 + value * 0.2, 0.81 + value * 0.2];
        }
        self.surface(rgb)
    }
    pub(crate) fn thumb(self) -> [f64; 3] {
        match self {
            Self::Dark => [0.46, 0.39, 0.53],
            Self::Light => [0.57, 0.44, 0.70],
        }
    }
    pub(crate) fn apply(self, ctx: &egui::Context) {
        let theme = if self == Self::Dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        let mut visuals = if self == Self::Dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = crate::canvas::color(self.surface([0.145; 3]));
        visuals.window_fill = crate::canvas::color(self.surface([0.2; 3]));
        visuals.extreme_bg_color = crate::canvas::color(self.surface([0.09; 3]));
        visuals.selection.bg_fill = crate::canvas::color(CORAL);
        visuals.selection.stroke = egui::Stroke::new(1., crate::canvas::color(PLUM));
        visuals.override_text_color = None;
        let ink = crate::canvas::color(self.ink([0.75; 3]));
        visuals.widgets.noninteractive.fg_stroke.color = ink;
        visuals.widgets.inactive.fg_stroke.color = ink;
        visuals.widgets.hovered.fg_stroke.color = ink;
        visuals.widgets.active.fg_stroke.color = ink;
        visuals.widgets.hovered.bg_stroke.color = crate::canvas::color(self.stroke(CORAL));
        visuals.widgets.active.bg_stroke.color = crate::canvas::color(self.stroke(ROSE));
        ctx.set_visuals(visuals);
    }
}
pub(crate) fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: LOGO.to_vec(),
        width: 128,
        height: 128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(rgb: [f64; 3]) -> f64 {
        let linear = rgb.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }
    #[test]
    fn default_theme_is_dark_and_text_keeps_contrast_in_both_themes() {
        assert!(Theme::default() == Theme::Dark);
        for theme in [Theme::Dark, Theme::Light] {
            for background in [[0.117; 3], [0.145; 3], [0.2; 3]] {
                let background_luminance = luminance(theme.surface(background));
                let control_luminance = luminance(theme.thumb());
                assert!(
                    (background_luminance.max(control_luminance) + 0.05)
                        / (background_luminance.min(control_luminance) + 0.05)
                        >= 3.
                );
                for text in [[0.74, 0.76, 0.77], [0.43, 0.45, 0.46]] {
                    let a = luminance(theme.surface(background));
                    let b = luminance(theme.ink(text));
                    assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
                }
            }
        }
    }
}
