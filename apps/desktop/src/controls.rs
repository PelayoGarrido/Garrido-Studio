use super::canvas::{Rect, rect};
pub(crate) struct Control {
    pub name: &'static str,
    pub tag: isize,
    pub r: Rect,
}
fn control(name: &'static str, tag: isize, x: f64, y: f64, w: f64, h: f64) -> Control {
    Control {
        name,
        tag,
        r: rect(x, y, w, h),
    }
}
pub(crate) fn toolbar() -> Vec<Control> {
    vec![
        control("Pattern mode", 9003, 462., 10., 82., 32.),
        control("Song mode", 9004, 548., 10., 72., 32.),
        control("Play", 1, 646., 8., 42., 36.),
        control("Stop", 2, 694., 8., 42., 36.),
        control("Record", 70, 742., 8., 42., 36.),
        control("Tempo down", 31, 804., 10., 28., 32.),
        control("Tempo up", 32, 898., 10., 28., 32.),
        control("Metronome", 9005, 948., 10., 38., 32.),
        control("Open", 4, 1480., 10., 76., 32.),
        control("Save", 3, 1564., 10., 76., 32.),
        control("Export", 5, 1648., 10., 92., 32.),
        control("Audio settings", 9041, 1752., 10., 100., 32.),
        control("Help", 9351, 1864., 10., 40., 32.),
        control("File", 2000, 16., 51., 42., 26.),
        control("Edit", 2001, 64., 51., 42., 26.),
        control("Add", 9, 112., 51., 42., 26.),
        control("Patterns", 2002, 160., 51., 72., 26.),
        control("View", 2003, 238., 51., 46., 26.),
        control("Options", 2004, 290., 51., 66., 26.),
        control("Tools", 2005, 362., 51., 48., 26.),
        control("Pattern selector", 2006, 440., 51., 176., 26.),
        control("Previous pattern", 83, 620., 51., 24., 26.),
        control("Next pattern", 84, 648., 51., 24., 26.),
        control("Arrange", 102, 718., 51., 108., 26.),
        control("Steps", 100, 834., 51., 90., 26.),
        control("Piano", 101, 932., 51., 90., 26.),
        control("Mixer", 103, 1030., 51., 90., 26.),
        control("Audio", 104, 1128., 51., 90., 26.),
        control("Automation", 105, 1226., 51., 126., 26.),
        control("Snap", 9030, 1380., 51., 124., 26.),
        control("Undo", 6, 1530., 51., 64., 26.),
        control("Effects", 40, 1604., 51., 78., 26.),
        control("Plugins", 77, 1692., 51., 80., 26.),
        control("About", 9352, 1782., 51., 70., 26.),
    ]
}
pub(crate) fn controls() -> Vec<Control> {
    let mut items = toolbar();
    items.extend([
        control("Browser audio", 9360, 10., 108., 58., 32.),
        control("Browser project", 9361, 74., 108., 58., 32.),
        control("Browser plugins", 9362, 138., 108., 58., 32.),
    ]);
    for (name, tag) in [
        ("Reverse", 50),
        ("Normalize", 51),
        ("Fade in", 52),
        ("Fade out", 53),
        ("Trim", 54),
        ("Stretch", 55),
        ("Pitch", 56),
    ] {
        items.push(control(name, tag, 0., 0., 73., 26.));
    }
    items
}
