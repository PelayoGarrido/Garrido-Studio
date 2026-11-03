use super::canvas::*;
use crate::app::Page;
use garrido_studio::model::*;
use std::cell::Cell;
pub(crate) const PIANO_TOOLS: [(&str, f64); 16] = [
    ("chevron-right", 9.),
    ("adjustments", 28.),
    ("magnet", 48.),
    ("music", 68.),
    ("link", 89.),
    ("arrow-back-up", 111.),
    ("pencil", 144.),
    ("brush", 165.),
    ("plus", 186.),
    ("eraser", 207.),
    ("volume-off", 228.),
    ("pencil", 249.),
    ("select", 270.),
    ("zoom-in", 291.),
    ("volume", 312.),
    ("volume", 339.),
];

#[derive(Clone, Copy)]
pub(crate) struct Panel {
    pub(crate) rect: Rect,
    pub(crate) visible: bool,
}
pub(crate) enum Drag {
    Window {
        index: usize,
        dx: f64,
        dy: f64,
    },
    Note {
        base: Box<Project>,
        pattern: usize,
        index: usize,
        indices: Vec<usize>,
        x: f64,
        y: f64,
        width: f64,
        row: f64,
        resize: bool,
    },
    Clip {
        base: Box<Project>,
        ids: Vec<garrido_studio::arrangement::ClipId>,
        x: f64,
        y: f64,
        width: f64,
        span: u32,
        resize: bool,
    },
    Selection {
        pattern: usize,
        origin: Point,
        current: Point,
        grid: Rect,
        base_key: u8,
        previous: Vec<usize>,
    },
    Automation {
        base: Box<Project>,
        seed: Box<Project>,
        lane: usize,
        point: usize,
        graph: Rect,
    },
    PanView {
        x: f64,
        start: u32,
        zoom: f64,
    },
    Gain {
        base: Box<Project>,
        track: usize,
        top: f64,
        height: f64,
    },
}
pub(crate) struct Workspace {
    pub(crate) panels: [Panel; 5],
    pub(crate) order: Vec<usize>,
    pub(crate) drag: Option<Drag>,
    pub(crate) size: (f64, f64),
    pub(crate) selected_notes: Vec<usize>,
    pub(crate) selected_clips: Vec<garrido_studio::arrangement::ClipId>,
    pub(crate) clip_clipboard: Option<garrido_studio::arrangement::Clipboard>,
    pub(crate) arrangement_cursor: u32,
    pub(crate) arrangement_lane: usize,
    pub(crate) clipboard: Option<garrido_studio::notes::Clipboard>,
    pub(crate) cursor: u32,
    pub(crate) automation_target: u8,
    pub(crate) automation_curve: Curve,
    pub(crate) piano_view: Cell<Option<(usize, u8)>>,
    pub(crate) piano_zoom: Cell<f64>,
    pub(crate) piano_start: Cell<u32>,
    pub(crate) piano_maximized: bool,
    pub(crate) piano_restore: Option<Rect>,
    pub(crate) browser_tab: u8,
    pub(crate) browser_expanded: bool,
    pub(crate) note_grid: u32,
    pub(crate) piano_erase: bool,
    pub(crate) piano_select: bool,
    pub(crate) ghost_notes: bool,
    pub(crate) arrangement_start: u32,
    pub(crate) arrangement_span: u32,
    pub(crate) arrangement_row: f64,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            panels: [Panel {
                rect: rect(0., 0., 1., 1.),
                visible: false,
            }; 5],
            order: vec![0, 1, 2, 3, 4],
            drag: None,
            size: (0., 0.),
            selected_notes: Vec::new(),
            selected_clips: Vec::new(),
            clip_clipboard: None,
            arrangement_cursor: 0,
            arrangement_lane: 0,
            clipboard: None,
            cursor: 0,
            automation_target: 0,
            automation_curve: Curve::Smooth,
            piano_view: Cell::new(None),
            piano_zoom: Cell::new(96.),
            piano_start: Cell::new(0),
            piano_maximized: false,
            piano_restore: None,
            browser_tab: 0,
            browser_expanded: false,
            note_grid: PPQ / 4,
            piano_erase: false,
            piano_select: false,
            ghost_notes: true,
            arrangement_start: 0,
            arrangement_span: 24 * BAR,
            arrangement_row: 0.,
        }
    }
}
impl Workspace {
    pub(crate) fn arrangement(&self, width: f64, height: f64) -> Arrangement {
        let ticks = self.arrangement_span.clamp(BAR, MAX_TICKS);
        let rows = ((height - 148.) / 48.).max(1.);
        Arrangement {
            grid: rect(481., 148., (width - 503.).max(1.), (height - 148.).max(1.)),
            start: self.arrangement_start.min(MAX_TICKS - ticks),
            ticks,
            first_row: self
                .arrangement_row
                .clamp(0., (MAX_LANES as f64 - rows).max(0.)),
        }
    }
    pub(crate) fn visible(&self, index: usize) -> bool {
        self.panels.get(index).is_some_and(|p| p.visible)
    }
    pub(crate) fn show(&mut self, page: Page) {
        let i = match page {
            Page::Steps => 0,
            Page::Piano => 1,
            Page::Mixer => 2,
            Page::Audio => 3,
            Page::Automation => 4,
            Page::Arrange => return,
        };
        self.panels[i].visible = true;
        self.order.retain(|v| *v != i);
        self.order.push(i);
    }
    pub(crate) fn reset_selection(&mut self) {
        self.selected_notes.clear();
        self.selected_clips.clear();
        self.piano_view.set(None);
        self.piano_start.set(0);
        self.cursor = 0;
    }
    pub(crate) fn browser_mode(&mut self, tab: u8) {
        self.browser_tab = tab;
        self.browser_expanded = tab == 1;
    }
    pub(crate) fn snap(&mut self, grid: u32) {
        self.note_grid = grid.max(1);
    }
    pub(crate) fn maximize_piano(&mut self) {
        self.piano_maximized = !self.piano_maximized;
        if self.piano_maximized {
            self.piano_restore = Some(self.panels[1].rect);
            self.panels[1].rect = rect(209., 82., self.size.0 - 209., self.size.1 - 82.);
            self.browser_expanded = true;
            self.browser_tab = 1;
        } else if let Some(r) = self.piano_restore.take() {
            self.panels[1].rect = r;
        }
    }
    pub(crate) fn audio_rect(&self) -> Rect {
        self.panels[3].rect
    }
    pub(crate) fn layout(&mut self, w: f64, h: f64) {
        if self.size == (w, h) {
            return;
        }
        let first = self.size.0 == 0.;
        self.size = (w, h);
        let positions = [
            rect(278., h - 380., 511., 247.),
            if self.piano_maximized {
                rect(209., 82., w - 209., h - 82.)
            } else {
                rect(w - 819., 176., 779., 719_f64.min(h - 200.))
            },
            rect(368., 176., w - 390., h - 236.),
            rect(368., 176., w - 390., h - 236.),
            rect(368., 176., w - 390., h - 236.),
        ];
        for (i, p) in self.panels.iter_mut().enumerate() {
            p.rect = positions[i];
            if first {
                p.visible = i < 2;
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Arrangement {
    pub(crate) grid: Rect,
    pub(crate) start: u32,
    pub(crate) ticks: u32,
    pub(crate) first_row: f64,
}
impl Arrangement {
    pub(crate) fn tick_at(self, x: f64) -> u32 {
        (self.start as f64 + (x - self.grid.origin.x) / self.grid.size.width * self.ticks as f64)
            .clamp(0., (MAX_TICKS - 1) as f64) as u32
    }
    pub(crate) fn lane_at(self, y: f64) -> usize {
        (self.first_row + (y - self.grid.origin.y) / 48.)
            .floor()
            .max(0.) as usize
    }
    pub(crate) fn x_at(self, tick: u32) -> f64 {
        self.grid.origin.x
            + (tick as f64 - self.start as f64) / self.ticks as f64 * self.grid.size.width
    }
    pub(crate) fn y_at(self, lane: usize) -> f64 {
        self.grid.origin.y + (lane as f64 - self.first_row) * 48.
    }
}
