use super::*;

const PIANO_TOOLS: [(&str, f64); 16] = [
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
mod reference;

use super::theme::{
    DIM, EDGE, GOLD, ICON, PANEL, TEXT, circle, icon, knob, line, outline, round, segments,
};

#[derive(Clone, Copy)]
struct Panel {
    rect: Rect,
    visible: bool,
}
enum Drag {
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
        index: usize,
        x: f64,
        y: f64,
        width: f64,
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
pub(super) struct Workspace {
    panels: [Panel; 5],
    order: Vec<usize>,
    drag: Option<Drag>,
    size: (f64, f64),
    pub(super) selected_notes: Vec<usize>,
    pub(super) clipboard: Option<garrido_studio::notes::Clipboard>,
    pub(super) cursor: u32,
    automation_target: u8,
    automation_curve: Curve,
    piano_view: Cell<Option<(usize, u8)>>,
    piano_zoom: Cell<f64>,
    piano_start: Cell<u32>,
    piano_maximized: bool,
    piano_restore: Option<Rect>,
    browser_tab: u8,
    browser_expanded: bool,
    note_grid: u32,
    piano_erase: bool,
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
        }
    }
}
impl Workspace {
    pub(super) fn visible(&self, index: usize) -> bool {
        self.panels.get(index).is_some_and(|p| p.visible)
    }
    pub(super) fn show(&mut self, page: Page) {
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
    pub(super) fn reset_selection(&mut self) {
        self.selected_notes.clear();
        self.piano_view.set(None);
        self.piano_start.set(0);
        self.cursor = 0;
    }
    pub(super) fn browser_mode(&mut self, tab: u8) {
        self.browser_tab = tab;
        self.browser_expanded = tab == 1;
    }
    pub(super) fn snap(&mut self, grid: u32) {
        self.note_grid = grid.max(1);
    }
    fn maximize_piano(&mut self) {
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
    pub(super) fn audio_rect(&self) -> Rect {
        self.panels[3].rect
    }
    fn layout(&mut self, w: f64, h: f64) {
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
fn inside(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x
        && p.x < r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y < r.origin.y + r.size.height
}
fn border(r: Rect) {
    round(r, 11., PANEL);
    outline(r, 11., 1., [0.12, 0.12, 0.12]);
}
fn text(s: &str, x: f64, y: f64, size: f64) {
    label(s, x, y, size.max(14.), TEXT, false);
}
fn name(key: u8) -> String {
    format!(
        "{}{}",
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ][(key % 12) as usize],
        key as i16 / 12 - 1
    )
}

pub(super) fn controls(view: Id) {
    for spec in theme::controls() {
        button(
            view,
            spec.name,
            spec.tag,
            spec.r.origin.x,
            spec.r.origin.y,
            spec.r.size.width,
        );
    }
    let views = send!(view, "subviews", Id);
    let count = send!(views, "count", usize);
    for i in 0..count {
        let control = send!(views,"objectAtIndex:",Id;i,usize);
        send!(control,"setBordered:",();0i8,i8);
        send!(control,"setTransparent:",();1i8,i8);
        if (50..=56).contains(&send!(control, "tag", isize)) {
            send!(control,"setHidden:",();1i8,i8);
        }
    }
}

impl State {
    pub(super) fn draw(&mut self) {
        self.draw_reference();
    }
    fn piano_geometry(&self, r: Rect) -> (Rect, u8) {
        let full = self.workspace.piano_maximized;
        let top = if full { 65. } else { 68. };
        let graph = rect(
            r.origin.x + if full { 73. } else { 78. },
            r.origin.y + top,
            r.size.width - if full { 96. } else { 107. },
            r.size.height - top - if full { 185. } else { 193. },
        );
        let rows = (graph.size.height / 18.).ceil().clamp(1., 128.) as u8;
        let pattern = self.pattern().unwrap_or(usize::MAX);
        let base = if let Some((_, key)) = self
            .workspace
            .piano_view
            .get()
            .filter(|(i, _)| *i == pattern)
        {
            key
        } else {
            let key = self
                .history
                .project
                .patterns
                .get(pattern)
                .and_then(|p| p.notes.iter().map(|n| n.key).min())
                .map_or(59, |key| key.saturating_sub(6));
            self.workspace.piano_view.set(Some((pattern, key)));
            key
        };
        (graph, base.min(128 - rows))
    }
    fn workspace_mixer(&self, r: Rect) {
        let start = (self.selected / 12) * 12;
        let count = self
            .history
            .project
            .tracks
            .len()
            .saturating_sub(start)
            .clamp(1, 12);
        let cw = (r.size.width - 15.) / count as f64;
        let top = r.origin.y + 52.;
        let height = r.size.height - 125.;
        for (i, t) in self
            .history
            .project
            .tracks
            .iter()
            .enumerate()
            .skip(start)
            .take(12)
        {
            let x = r.origin.x + 7. + (i - start) as f64 * cw;
            fill(
                rect(x, top - 25., cw - 3., r.size.height - 32.),
                [0.16, 0.17, 0.175],
            );
            text(
                &t.name.chars().take(10).collect::<String>(),
                x + 5.,
                top - 18.,
                10.,
            );
            fill(rect(x + cw * 0.48, top, 3., height), [0.1, 0.11, 0.12]);
            let yy = top + (1. - t.gain as f64 / 2.) * height;
            fill(
                rect(x + cw * 0.25, yy, cw * 0.5, 6.),
                if i == self.selected { GOLD } else { DIM },
            );
            text(
                &format!("{:.1}", 20. * t.gain.max(1e-6).log10()),
                x + 4.,
                top + height + 12.,
                9.,
            );
            text(
                if t.mute { "M ON" } else { "M" },
                x + 5.,
                top + height + 34.,
                10.,
            );
            text(
                if t.solo { "S ON" } else { "S" },
                x + cw * 0.55,
                top + height + 34.,
                10.,
            );
        }
    }
    fn automation_target(&self) -> Target {
        match self.workspace.automation_target {
            0 => Target::Gain(self.selected),
            1 => Target::Pan(self.selected),
            _ => Target::Master,
        }
    }
    fn automation_graph(&self, r: Rect) -> Rect {
        rect(
            r.origin.x + 48.,
            r.origin.y + 62.,
            r.size.width - 65.,
            r.size.height - 102.,
        )
    }
    fn workspace_automation(&self, r: Rect) {
        let target = self.automation_target();
        let graph = self.automation_graph(r);
        let end = self.history.project.bars as u32 * BAR;
        let lane = self
            .history
            .project
            .automation
            .iter()
            .find(|a| a.target == target);
        let curve = lane.map_or(self.workspace.automation_curve, |a| a.curve);
        for (i, title) in ["Gain", "Pan", "Master"].iter().enumerate() {
            label(
                title,
                r.origin.x + 12. + i as f64 * 68.,
                r.origin.y + 29.,
                11.,
                if self.workspace.automation_target as usize == i {
                    GOLD
                } else {
                    TEXT
                },
                false,
            );
        }
        text(
            &format!(
                "Curve: {}",
                ["Hold", "Linear", "Smooth", "Exponential"][curve as usize]
            ),
            r.origin.x + 235.,
            r.origin.y + 29.,
            10.,
        );
        fill(graph, [0.12, 0.13, 0.135]);
        let (low, high) = automation_range(target);
        for i in 0..=4 {
            let y = graph.origin.y + i as f64 * graph.size.height / 4.;
            fill(
                rect(graph.origin.x, y, graph.size.width, 0.5),
                [0.20, 0.21, 0.22],
            );
            text(
                &format!("{:.2}", high - (high - low) * i as f32 / 4.),
                r.origin.x + 5.,
                y - 5.,
                9.,
            );
        }
        for bar in 0..=self.history.project.bars {
            let x =
                graph.origin.x + bar as f64 / self.history.project.bars as f64 * graph.size.width;
            fill(rect(x, graph.origin.y, 0.5, graph.size.height), EDGE);
            if self.history.project.bars <= 32 || bar % 8 == 0 {
                text(&(bar + 1).to_string(), x + 2., graph.origin.y - 17., 9.);
            }
        }
        if let Some(lane) = lane {
            let mut previous = None;
            for px in 0..graph.size.width as usize {
                let v = lane.at(px as f64 / graph.size.width * end as f64);
                let y =
                    graph.origin.y + (1. - ((v - low) / (high - low)) as f64) * graph.size.height;
                let old = previous.unwrap_or(y);
                fill(
                    rect(
                        graph.origin.x + px as f64,
                        old.min(y),
                        1.,
                        (old - y).abs().max(1.),
                    ),
                    GOLD,
                );
                previous = Some(y);
            }
            for &(tick, v) in &lane.points {
                let x = graph.origin.x + tick as f64 / end as f64 * graph.size.width;
                let y =
                    graph.origin.y + (1. - ((v - low) / (high - low)) as f64) * graph.size.height;
                fill(rect(x - 3., y - 3., 6., 6.), [1., 0.91, 0.59]);
            }
        }
        text(
            "Click to add · drag to move · right-click to delete",
            r.origin.x + 12.,
            r.origin.y + r.size.height - 25.,
            10.,
        );
    }
    fn automation_mouse(&mut self, p: Point, r: Rect, right: bool) {
        if p.y < r.origin.y + 50. {
            if p.x < r.origin.x + 215. {
                self.workspace.automation_target =
                    ((p.x - r.origin.x - 5.) / 68.).clamp(0., 2.) as u8;
            } else {
                let target = self.automation_target();
                let old = self
                    .history
                    .project
                    .automation
                    .iter()
                    .find(|a| a.target == target)
                    .map_or(self.workspace.automation_curve, |a| a.curve);
                let next = [
                    Curve::Hold,
                    Curve::Linear,
                    Curve::Smooth,
                    Curve::Exponential,
                ][(old as usize + 1) % 4];
                self.workspace.automation_curve = next;
                let result = self.history.edit(|p| {
                    if let Some(a) = p.automation.iter_mut().find(|a| a.target == target) {
                        a.curve = next;
                    }
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
            self.redraw();
            return;
        }
        let graph = self.automation_graph(r);
        if !inside(graph, p) {
            return;
        }
        let target = self.automation_target();
        if matches!(target,Target::Gain(i)|Target::Pan(i) if i>=self.history.project.tracks.len()) {
            return;
        }
        let end = self.history.project.bars as u32 * BAR;
        let tick = (((p.x - graph.origin.x) / graph.size.width * end as f64 / (PPQ / 4) as f64)
            .round() as u32
            * (PPQ / 4))
            .min(end);
        let (low, high) = automation_range(target);
        let value = low
            + (1. - ((p.y - graph.origin.y) / graph.size.height).clamp(0., 1.) as f32)
                * (high - low);
        let base = self.history.project.clone();
        let mut seed = base.clone();
        let lane = if let Some(i) = seed.automation.iter().position(|a| a.target == target) {
            i
        } else {
            if right || seed.automation.len() >= 64 {
                return;
            }
            seed.automation.push(Automation {
                target,
                curve: self.workspace.automation_curve,
                points: Vec::new(),
            });
            seed.automation.len() - 1
        };
        let points = &mut seed.automation[lane].points;
        let near = points
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.0.abs_diff(tick))
            .filter(|(_, p)| p.0.abs_diff(tick) as f64 / end as f64 * graph.size.width < 7.)
            .map(|(i, _)| i);
        if right {
            let Some(index) = near else {
                return;
            };
            points.remove(index);
            if points.is_empty() {
                seed.automation.remove(lane);
            }
            let result = self.history.edit(|p| {
                *p = seed;
                Ok(())
            });
            if result.is_ok() {
                self.changed();
            }
            self.result(result);
            return;
        }
        let point = if let Some(i) = near {
            i
        } else {
            if points.len() >= 4096 {
                return;
            }
            let i = points.partition_point(|p| p.0 < tick);
            points.insert(i, (tick, value));
            i
        };
        if seed.validate().is_err() {
            return;
        }
        self.stop();
        self.history.project = seed.clone();
        self.workspace.drag = Some(Drag::Automation {
            base: Box::new(base),
            seed: Box::new(seed),
            lane,
            point,
            graph,
        });
        self.redraw();
    }
    fn workspace_audio(&self, r: Rect) {
        let Some(s) = self
            .history
            .project
            .tracks
            .get(self.selected)
            .and_then(|t| t.sample.as_ref())
        else {
            text(
                "Load a WAV or record audio into the selected channel.",
                r.origin.x + 20.,
                r.origin.y + 80.,
                12.,
            );
            return;
        };
        let g = rect(
            r.origin.x + 10.,
            r.origin.y + 55.,
            r.size.width - 20.,
            r.size.height - 100.,
        );
        fill(g, [0.12, 0.13, 0.135]);
        if self.selection.1 > self.selection.0 {
            fill(
                rect(
                    g.origin.x + g.size.width * self.selection.0,
                    g.origin.y,
                    g.size.width * (self.selection.1 - self.selection.0),
                    g.size.height,
                ),
                [0.24, 0.25, 0.20],
            );
        }
        for px in 0..g.size.width as usize {
            let a = px * s.frames.len() / g.size.width as usize;
            let b = ((px + 1) * s.frames.len() / g.size.width as usize)
                .max(a + 1)
                .min(s.frames.len());
            if a >= b {
                continue;
            }
            let peak = s.frames[a..b]
                .iter()
                .flatten()
                .fold(0f32, |a, &b| a.max(b.abs()));
            fill(
                rect(
                    g.origin.x + px as f64,
                    g.origin.y + g.size.height * (0.5 - peak as f64 * 0.45),
                    1.,
                    (peak as f64 * g.size.height * 0.9).max(1.),
                ),
                GOLD,
            );
        }
        text(
            &format!(
                "{:.2}s / {} Hz / original preserved",
                s.frames.len() as f64 / s.rate as f64,
                s.rate
            ),
            r.origin.x + 12.,
            r.origin.y + r.size.height - 28.,
            10.,
        );
    }
    fn event_point(&self, event: Id) -> Point {
        let p = send!(event, "locationInWindow", Point);
        let p = send!(self.view,"convertPoint:fromView:",Point;p,Point;ptr::null_mut(),Id);
        Point {
            x: p.x / self.scale,
            y: p.y / self.scale,
        }
    }
    pub(super) fn mouse(&mut self, event: Id, right: bool) {
        send!(send!(self.view,"window",Id),"makeFirstResponder:",i8;self.view,Id);
        let p = self.event_point(event);
        if p.y < 40. && ((p.x >= 73. && p.x < 378.) || p.x > 1470.) && !right {
            send!(send!(self.view,"window",Id),"performWindowDragWithEvent:",();event,Id);
            return;
        }
        for i in self.workspace.order.clone().into_iter().rev() {
            let panel = self.workspace.panels[i];
            if !panel.visible || !inside(panel.rect, p) {
                continue;
            }
            self.workspace.order.retain(|v| *v != i);
            self.workspace.order.push(i);
            self.page = match i {
                0 => Page::Steps,
                1 => Page::Piano,
                2 => Page::Mixer,
                3 => Page::Audio,
                _ => Page::Automation,
            };
            self.sync_tools();
            let r = panel.rect;
            if p.y < r.origin.y + 26. {
                if p.x > r.origin.x + r.size.width - 25. {
                    self.workspace.panels[i].visible = false;
                } else if i == 1
                    && ((p.x > r.origin.x + r.size.width - 49.
                        && p.x < r.origin.x + r.size.width - 27.)
                        || send!(event, "clickCount", isize) >= 2)
                {
                    self.workspace.maximize_piano();
                } else if i == 1 && p.x < r.origin.x + 355. {
                    let item = PIANO_TOOLS.iter().position(|(_, x)| {
                        p.x >= r.origin.x + x - 1. && p.x < r.origin.x + x + 19.
                    });
                    match item {
                        Some(0 | 1) => self.action(2005),
                        Some(2) => self.action(9030),
                        Some(3) => self.action(2002),
                        Some(5) => self.action(6),
                        Some(6 | 7) => self.workspace.piano_erase = false,
                        Some(9) => self.workspace.piano_erase = true,
                        Some(12) => self.action(440),
                        Some(13) => self
                            .workspace
                            .piano_zoom
                            .set((self.workspace.piano_zoom.get() * 1.25).min(384.)),
                        _ => {}
                    }
                } else {
                    self.workspace.drag = Some(Drag::Window {
                        index: i,
                        dx: p.x - r.origin.x,
                        dy: p.y - r.origin.y,
                    });
                }
                self.redraw();
                return;
            }
            let result = match i {
                0 => {
                    if p.y > r.origin.y + r.size.height - 30.
                        && p.x > r.origin.x + 130.
                        && p.x < r.origin.x + 190.
                    {
                        self.action(9);
                        return;
                    }
                    let row = ((p.y - r.origin.y - 48.) / 28.).floor();
                    if row < 0. {
                        return;
                    }
                    let track = (self.selected / 6) * 6 + row as usize;
                    if track >= self.history.project.tracks.len() {
                        return;
                    }
                    if self.selected != track {
                        self.selected_pattern = None;
                        self.workspace.selected_notes.clear();
                    }
                    self.selected = track;
                    if right {
                        self.history.edit(|p| {
                            p.tracks[track].mute = !p.tracks[track].mute;
                            Ok(())
                        })
                    } else if p.x >= r.origin.x + 229. {
                        let step = ((p.x - r.origin.x - 229.) / 16.).floor().clamp(0., 15.) as u32;
                        let Some(pattern) = self.pattern() else {
                            return;
                        };
                        self.history.edit(|p| {
                            let pat = &mut p.patterns[pattern];
                            let tick = step * PPQ / 4;
                            if tick >= pat.length {
                                return Ok(());
                            }
                            if pat.notes.iter().any(|n| n.tick / (PPQ / 4) == step) {
                                pat.notes.retain(|n| n.tick / (PPQ / 4) != step);
                            } else {
                                let key = if track < 4 {
                                    [36, 38, 42, 36][track]
                                } else {
                                    60
                                };
                                pat.notes.push(Note::new(
                                    tick,
                                    (PPQ / 4).min(pat.length - tick),
                                    key,
                                    0.65,
                                ));
                            }
                            Ok(())
                        })
                    } else {
                        self.redraw();
                        return;
                    }
                }
                1 => {
                    let right = right || self.workspace.piano_erase;
                    let (g, base) = self.piano_geometry(r);
                    let Some(pattern) = self.pattern() else {
                        return;
                    };
                    if p.y < g.origin.y && p.y > r.origin.y + 26. && p.x >= g.origin.x {
                        self.workspace.drag = Some(Drag::PanView {
                            x: p.x,
                            start: self.workspace.piano_start.get(),
                            zoom: self.workspace.piano_zoom.get(),
                        });
                        return;
                    }
                    if p.y > g.origin.y + g.size.height + 9.
                        && p.y < r.origin.y + r.size.height - 5.
                        && p.x >= g.origin.x
                        && p.x < g.origin.x + g.size.width
                    {
                        let pat = &self.history.project.patterns[pattern];
                        let tick = self.workspace.piano_start.get()
                            + ((p.x - g.origin.x) / self.workspace.piano_zoom.get() * PPQ as f64)
                                as u32;
                        let threshold =
                            (PPQ as f64 / self.workspace.piano_zoom.get() * 5.).ceil() as u32;
                        if let Some(index) = pat
                            .notes
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, n)| n.tick.abs_diff(tick))
                            .filter(|(_, n)| n.tick.abs_diff(tick) <= threshold)
                            .map(|(i, _)| i)
                        {
                            let bottom = r.origin.y + r.size.height - 7.;
                            let velocity = ((bottom - p.y)
                                / (bottom - g.origin.y - g.size.height - 34.))
                                .clamp(0., 1.) as f32;
                            let result = self.history.edit(|p| {
                                p.patterns[pattern].notes[index].velocity = velocity;
                                Ok(())
                            });
                            if result.is_ok() {
                                self.changed();
                            }
                            self.result(result);
                        }
                        return;
                    }
                    if !inside(g, p) {
                        return;
                    }
                    let pat = &self.history.project.patterns[pattern];
                    let grid = self.workspace.note_grid;
                    let tick = ((self.workspace.piano_start.get()
                        + ((p.x - g.origin.x) / self.workspace.piano_zoom.get() * PPQ as f64)
                            as u32)
                        / grid
                        * grid)
                        .min(MAX_TICKS - 1);
                    self.workspace.cursor = tick;
                    let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
                    let key = base + rows - 1 - ((p.y - g.origin.y) / 18.) as u8;
                    if let Some(index) = pat
                        .notes
                        .iter()
                        .position(|n| n.key == key && n.tick <= tick && n.tick + n.length > tick)
                    {
                        if right {
                            self.workspace.selected_notes.clear();
                            self.history.edit(|p| {
                                p.patterns[pattern].notes.remove(index);
                                Ok(())
                            })
                        } else {
                            let flags = send!(event, "modifierFlags", usize);
                            if flags & (1 << 20) != 0 {
                                if self.workspace.selected_notes.contains(&index) {
                                    self.workspace.selected_notes.retain(|i| *i != index);
                                } else {
                                    self.workspace.selected_notes.push(index);
                                }
                                self.redraw();
                                return;
                            }
                            if !self.workspace.selected_notes.contains(&index) {
                                self.workspace.selected_notes = vec![index];
                            }
                            let resize = flags & (1 << 17) != 0;
                            self.workspace.drag = Some(Drag::Note {
                                base: Box::new(self.history.project.clone()),
                                pattern,
                                index,
                                indices: self.workspace.selected_notes.clone(),
                                x: p.x,
                                y: p.y,
                                width: self.workspace.piano_zoom.get(),
                                row: 18.,
                                resize,
                            });
                            return;
                        }
                    } else if !right {
                        self.workspace.selected_notes.clear();
                        self.history.edit(|p| {
                            let length = (PPQ / 4).min(MAX_TICKS - tick);
                            p.patterns[pattern].length = p.patterns[pattern]
                                .length
                                .max((tick + length).div_ceil(BAR) * BAR);
                            p.patterns[pattern]
                                .notes
                                .push(Note::new(tick, length, key, 0.65));
                            Ok(())
                        })
                    } else {
                        return;
                    }
                }
                2 => {
                    let start = (self.selected / 12) * 12;
                    let count = self
                        .history
                        .project
                        .tracks
                        .len()
                        .saturating_sub(start)
                        .clamp(1, 12);
                    let cw = (r.size.width - 15.) / count as f64;
                    let track = start + ((p.x - r.origin.x - 7.) / cw).floor().max(0.) as usize;
                    if track >= self.history.project.tracks.len() {
                        return;
                    }
                    self.selected = track;
                    if right {
                        self.routing_menu(p);
                        self.redraw();
                        return;
                    }
                    let top = r.origin.y + 52.;
                    let height = r.size.height - 125.;
                    if p.y >= top && p.y < top + height {
                        self.workspace.drag = Some(Drag::Gain {
                            base: Box::new(self.history.project.clone()),
                            track,
                            top,
                            height,
                        });
                        self.drag(event);
                        return;
                    }
                    let point = p;
                    self.history.edit(|p| {
                        if (point.y - top - height - 34.).abs() < 14. {
                            if (point.x - r.origin.x - 7.) % cw < cw * 0.5 {
                                p.tracks[track].mute = !p.tracks[track].mute;
                            } else {
                                p.tracks[track].solo = !p.tracks[track].solo;
                            }
                        }
                        Ok(())
                    })
                }
                4 => {
                    self.automation_mouse(p, r, right);
                    return;
                }
                _ => {
                    let at = ((p.x - r.origin.x - 10.) / (r.size.width - 20.)).clamp(0., 1.);
                    if let Some(anchor) = self.anchor.take() {
                        self.selection = (anchor.min(at), anchor.max(at));
                    } else {
                        self.anchor = Some(at);
                        self.selection = (at, at);
                    }
                    self.redraw();
                    return;
                }
            };
            if result.is_ok() {
                self.changed();
            }
            self.result(result);
            return;
        }
        if p.x >= 209. && p.x < 350. && p.y >= 154. {
            let start = self.pattern().unwrap_or(0).saturating_sub(20);
            self.select_pattern(start + ((p.y - 154.) / 32.) as usize);
            return;
        }
        if p.x < 205. {
            if (142. ..166.).contains(&p.y) {
                self.workspace.browser_expanded = !self.workspace.browser_expanded;
                self.redraw();
                return;
            }
            if self.workspace.browser_expanded && p.y >= 190. {
                let count = self
                    .history
                    .project
                    .patterns
                    .len()
                    .min(((self.height - 330.) / 23.).max(0.) as usize);
                let row = ((p.y - 190.) / 23.) as usize;
                if row < count {
                    self.select_pattern(row);
                    if send!(event, "clickCount", isize) >= 2 {
                        self.workspace.show(Page::Piano);
                    }
                    return;
                }
                if row == count {
                    self.action(40);
                } else if row == count + 1 {
                    self.action(9);
                } else if row == count + 2 {
                    self.action(8);
                } else {
                    self.action(71);
                }
            } else if p.y >= 166. && p.y < 189. {
                self.action(71);
            }
            return;
        }
        if p.x >= 481. && p.x < 1899. && p.y >= 148. && p.y < self.height {
            let track = ((p.y - 148.) / 48.) as usize;
            if track >= self.history.project.tracks.len() {
                return;
            }
            self.selected = track;
            let end = self.history.project.bars.max(24) as f64 * BAR as f64;
            let gx = 481.;
            let width = self.width - gx - 22.;
            let grid = self.workspace.note_grid;
            let tick = (((p.x - gx) / width * end) as u32 / grid) * grid;
            let existing = self
                .history
                .project
                .clips
                .iter()
                .position(|c| c.track == track && c.tick <= tick && c.tick + c.length > tick);
            if let Some(index) = existing {
                self.selected_pattern = Some(self.history.project.clips[index].pattern);
                self.workspace.selected_notes.clear();
                if right {
                    let result = self.history.edit(|p| {
                        p.clips.remove(index);
                        Ok(())
                    });
                    if result.is_ok() {
                        self.changed();
                    }
                    self.result(result);
                } else {
                    self.workspace.drag = Some(Drag::Clip {
                        base: Box::new(self.history.project.clone()),
                        index,
                        x: p.x,
                        y: p.y,
                        width,
                    });
                }
            } else if !right
                && tick + BAR <= MAX_TICKS
                && let Some(pattern) = self.pattern()
            {
                let result = self.history.edit(|p| {
                    p.bars = p.bars.max((tick + BAR).div_ceil(BAR) as u16);
                    p.clips.push(Clip {
                        offset: 0,
                        muted: false,
                        group: 0,
                        track,
                        pattern,
                        tick,
                        length: BAR,
                    });
                    Ok(())
                });
                if result.is_ok() {
                    self.changed();
                }
                self.result(result);
            }
        }
        self.redraw();
    }
    pub(super) fn drag(&mut self, event: Id) {
        let p = self.event_point(event);
        let Some(drag) = self.workspace.drag.as_ref() else {
            return;
        };
        let mut next = match drag {
            Drag::Window { .. } | Drag::PanView { .. } => None,
            Drag::Note { base, .. } | Drag::Clip { base, .. } | Drag::Gain { base, .. } => {
                Some(base.as_ref().clone())
            }
            Drag::Automation { seed, .. } => Some(seed.as_ref().clone()),
        };
        match drag {
            Drag::Window { index, dx, dy } => {
                let r = &mut self.workspace.panels[*index].rect;
                r.origin.x = (p.x - dx).clamp(209., self.width - 120.);
                r.origin.y = (p.y - dy).clamp(82., self.height - 60.);
            }
            Drag::Note {
                base,
                pattern,
                index,
                indices,
                x,
                y,
                width,
                row,
                resize,
            } => {
                if let Some(next) = &mut next {
                    let pat = &mut next.patterns[*pattern];
                    let old = base.patterns[*pattern].notes[*index];
                    let grid = self.workspace.note_grid as f64;
                    let delta = (((p.x - x) / width * PPQ as f64 / grid).round() * grid) as i64;
                    if *resize {
                        for i in indices {
                            let old = base.patterns[*pattern].notes[*i];
                            pat.notes[*i].length = (old.length as i64 + delta)
                                .clamp(1, (pat.length - old.tick) as i64)
                                as u32;
                        }
                    } else {
                        let starts = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].tick)
                            .min()
                            .unwrap_or(old.tick);
                        let ends = indices
                            .iter()
                            .map(|i| {
                                let n = base.patterns[*pattern].notes[*i];
                                n.tick + n.length
                            })
                            .max()
                            .unwrap_or(old.tick + old.length);
                        let min_key = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].key)
                            .min()
                            .unwrap_or(old.key);
                        let max_key = indices
                            .iter()
                            .map(|i| base.patterns[*pattern].notes[*i].key)
                            .max()
                            .unwrap_or(old.key);
                        let delta = delta.clamp(-(starts as i64), (pat.length - ends) as i64);
                        let pitch = (-((p.y - y) / row).round() as i64)
                            .clamp(-(min_key as i64), 127 - max_key as i64);
                        for i in indices {
                            let old = base.patterns[*pattern].notes[*i];
                            pat.notes[*i].tick = (old.tick as i64 + delta) as u32;
                            pat.notes[*i].key = (old.key as i64 + pitch) as u8;
                        }
                    }
                }
            }
            Drag::Clip {
                base,
                index,
                x,
                y,
                width,
            } => {
                if let Some(next) = &mut next {
                    let old = &base.clips[*index];
                    let delta = (((p.x - x) / width * base.bars.max(24) as f64 * BAR as f64
                        / self.workspace.note_grid as f64)
                        .round() as i64)
                        * self.workspace.note_grid as i64;
                    let c = &mut next.clips[*index];
                    c.tick =
                        (old.tick as i64 + delta).clamp(0, (MAX_TICKS - c.length) as i64) as u32;
                    c.track = (old.track as i64 + ((p.y - y) / 48.).round() as i64)
                        .clamp(0, base.tracks.len().saturating_sub(1) as i64)
                        as usize;
                    next.bars = next.bars.max((c.tick + c.length).div_ceil(BAR) as u16);
                }
            }
            Drag::Automation {
                seed,
                lane,
                point,
                graph,
                ..
            } => {
                if let Some(next) = &mut next {
                    let end = seed.bars as u32 * BAR;
                    let points = &seed.automation[*lane].points;
                    let min = if *point == 0 {
                        0
                    } else {
                        points[*point - 1].0 + 1
                    };
                    let max = points.get(*point + 1).map_or(end, |p| p.0 - 1);
                    let tick = ((((p.x - graph.origin.x) / graph.size.width).clamp(0., 1.)
                        * end as f64
                        / (PPQ / 4) as f64)
                        .round() as u32
                        * (PPQ / 4))
                        .clamp(min, max);
                    let (low, high) = automation_range(seed.automation[*lane].target);
                    let value = low
                        + (1. - ((p.y - graph.origin.y) / graph.size.height).clamp(0., 1.) as f32)
                            * (high - low);
                    next.automation[*lane].points[*point] = (tick, value);
                }
            }
            Drag::PanView { x, start, zoom } => self.workspace.piano_start.set(
                (*start as f64 + (p.x - x) / zoom * PPQ as f64).clamp(0., (MAX_TICKS - BAR) as f64)
                    as u32,
            ),
            Drag::Gain {
                track, top, height, ..
            } => {
                if let Some(next) = &mut next {
                    next.tracks[*track].gain =
                        ((1. - (p.y - top) / height) * 2.).clamp(0., 2.) as f32;
                }
            }
        }
        self.sync_tools();
        if let Some(next) = next
            && next.validate().is_ok()
        {
            self.stop();
            self.history.project = next;
        }
        self.redraw();
    }
    pub(super) fn scroll(&mut self, event: Id) {
        let point = self.event_point(event);
        let panel = self.workspace.panels[1];
        if panel.visible && inside(panel.rect, point) {
            let (g, key) = self.piano_geometry(panel.rect);
            let delta = send!(event, "scrollingDeltaY", f64);
            let dx = send!(event, "scrollingDeltaX", f64);
            let flags = send!(event, "modifierFlags", usize);
            if !delta.is_finite() || !dx.is_finite() {
                return;
            }
            if flags & (1 << 20) != 0 {
                self.workspace.piano_zoom.set(
                    (self.workspace.piano_zoom.get() * (1. + delta * 0.02).clamp(0.5, 2.))
                        .clamp(24., 384.),
                );
            } else if flags & (1 << 17) != 0 || dx.abs() > delta.abs() {
                let delta = if dx.abs() > delta.abs() { dx } else { delta };
                self.workspace.piano_start.set(
                    (self.workspace.piano_start.get() as f64
                        - delta / self.workspace.piano_zoom.get() * PPQ as f64)
                        .clamp(0., (MAX_TICKS - BAR) as f64) as u32,
                );
            } else {
                let rows = (g.size.height / 18.).ceil().clamp(1., 128.) as u8;
                let shift = (delta * 0.5).round().clamp(-12., 12.) as i16;
                self.workspace.piano_view.set(Some((
                    self.pattern().unwrap_or(usize::MAX),
                    (key as i16 + shift).clamp(0, (128 - rows) as i16) as u8,
                )));
            }
            self.redraw();
        }
    }
    pub(super) fn release(&mut self) {
        let Some(drag) = self.workspace.drag.take() else {
            return;
        };
        let base = match drag {
            Drag::Window { .. } | Drag::PanView { .. } => return,
            Drag::Note { base, .. }
            | Drag::Clip { base, .. }
            | Drag::Gain { base, .. }
            | Drag::Automation { base, .. } => base,
        };
        let next = self.history.project.clone();
        self.history.project = *base;
        let result = self.history.edit(|p| {
            *p = next;
            Ok(())
        });
        if result.is_ok() {
            self.changed();
        }
        self.result(result);
    }
}

fn automation_range(target: Target) -> (f32, f32) {
    match target {
        Target::Pan(_) => (-1., 1.),
        Target::Gain(_) => (0., 2.),
        Target::Master => (0., 1.),
    }
}
