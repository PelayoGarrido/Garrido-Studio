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
pub(crate) fn controls() -> Vec<Control> {
    let mut items = vec![
        control("Minimize", 9000, 3., 8., 19., 25.),
        control("Maximize", 9001, 24., 8., 19., 25.),
        control("Close", 9002, 44., 8., 19., 25.),
        control("Pattern mode", 9003, 386., 3., 32., 16.),
        control("Song mode", 9004, 386., 19., 32., 16.),
        control("Play", 1, 424., 4., 25., 30.),
        control("Stop", 2, 454., 4., 25., 30.),
        control("Record", 70, 487., 4., 25., 30.),
        control("Tempo down", 31, 554., 18., 15., 16.),
        control("Tempo up", 32, 554., 3., 15., 15.),
        control("Metronome", 9005, 580., 3., 31., 32.),
        control("Undo", 6, 1212., 4., 29., 30.),
        control("Cut", 531, 1244., 4., 29., 30.),
        control("Record input", 9350, 1276., 4., 29., 30.),
        control("Help", 9351, 1308., 4., 29., 30.),
        control("Save", 3, 1340., 4., 29., 30.),
        control("Open", 4, 1372., 4., 29., 30.),
        control("About", 9352, 1404., 4., 29., 30.),
        control("Export WAV", 5, 1436., 4., 29., 30.),
        control("FILE", 2000, 4., 45., 27., 31.),
        control("EDIT", 2001, 32., 45., 25., 31.),
        control("ADD", 9, 58., 45., 26., 31.),
        control("PATTERNS", 2002, 85., 45., 53., 31.),
        control("VIEW", 2003, 139., 45., 31., 31.),
        control("OPTIONS", 2004, 171., 45., 50., 31.),
        control("TOOLS", 2005, 222., 45., 35., 31.),
        control("HELP", 9353, 258., 45., 33., 31.),
        control("Channel rack", 100, 580., 45., 31., 32.),
        control("Piano roll", 101, 613., 45., 31., 32.),
        control("Mixer", 103, 646., 45., 30., 32.),
        control("Automation", 105, 679., 45., 30., 32.),
        control("Effects", 40, 713., 45., 30., 32.),
        control("Snap", 9030, 746., 45., 86., 32.),
        control("Pattern selector", 2006, 851., 45., 98., 32.),
        control("Previous pattern", 83, 836., 45., 15., 32.),
        control("Next pattern", 84, 950., 45., 19., 32.),
        control("Channel rack", 9301, 971., 45., 30., 32.),
        control("Piano roll", 9302, 1004., 45., 30., 32.),
        control("Playlist", 102, 1037., 45., 30., 32.),
        control("Mixer", 9303, 1070., 45., 30., 32.),
        control("Audio editor", 104, 1103., 45., 30., 32.),
        control("Open session", 9304, 1136., 45., 30., 32.),
        control("Tools", 9305, 1169., 45., 30., 32.),
        control("Plugins", 71, 1202., 45., 30., 32.),
        control("Audio tools", 9306, 1235., 45., 30., 32.),
        control("Browser audio", 9360, 4., 107., 33., 32.),
        control("Browser project", 9361, 44., 107., 33., 32.),
        control("Browser plugins", 9362, 84., 107., 33., 32.),
    ];
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
