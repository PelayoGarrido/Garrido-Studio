use super::*;

pub(super) const GOLD: [f64; 3] = [1., 0.86, 0.35];
pub(super) const TEXT: [f64; 3] = [0.74, 0.76, 0.77];
pub(super) const DIM: [f64; 3] = [0.43, 0.45, 0.46];
pub(super) const EDGE: [f64; 3] = [0.12, 0.12, 0.12];
pub(super) const PANEL: [f64; 3] = [0.20, 0.20, 0.20];
pub(super) const ICON: [f64; 3] = [0.69, 0.74, 0.78];

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontManagerRegisterFontsForURL(url: Id, scope: u32, error: *mut Id) -> bool;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Id);
}

pub(super) fn fonts() -> Result<()> {
    let exe = std::env::current_exe()?;
    let parent = exe
        .parent()
        .ok_or(Error::Invalid("Falta carpeta de aplicación"))?;
    let candidates = [
        parent.parent().unwrap_or(parent).join("Resources/fonts"),
        parent.join("fonts"),
    ];
    let folder = candidates
        .iter()
        .find(|p| p.join("Cuprum.ttf").is_file() && p.join("Tabler.ttf").is_file())
        .ok_or(Error::Invalid("Faltan las fuentes de la aplicación"))?;
    for (file, name) in [
        ("Cuprum.ttf", "Cuprum-Regular"),
        ("Tabler.ttf", "tabler-icons"),
    ] {
        let path = folder.join(file);
        let path = path
            .to_str()
            .ok_or(Error::Invalid("Ruta de fuente no válida"))?;
        let url = send!(class(c"NSURL"),"fileURLWithPath:",Id;string(path),Id);
        let mut error = ptr::null_mut();
        // SAFETY: fuentes propias del bundle, verificadas al empaquetar; ámbito del proceso.
        unsafe {
            CTFontManagerRegisterFontsForURL(url, 1, &mut error);
            if !error.is_null() {
                CFRelease(error);
            }
        }
        if send!(class(c"NSFont"),"fontWithName:size:",Id;string(name),Id;14.,f64).is_null() {
            return Err(Error::Unsupported(
                "No se pudo registrar la fuente de interfaz",
            ));
        }
    }
    Ok(())
}
pub(super) fn font(size: f64, _bold: bool) -> Id {
    let font = send!(class(c"NSFont"),"fontWithName:size:",Id;string("Cuprum-Regular"),Id;size,f64);
    if font.is_null() {
        return send!(class(c"NSFont"),"systemFontOfSize:",Id;size,f64);
    }
    font
}
pub(super) fn icon(name: &str, x: f64, y: f64, size: f64, tint: [f64; 3]) {
    let font = send!(class(c"NSFont"),"fontWithName:size:",Id;string("tabler-icons"),Id;size,f64);
    let attrs = send!(class(c"NSMutableDictionary"), "dictionary", Id);
    // SAFETY: claves constantes exportadas por AppKit.
    let (font_key, color_key) = unsafe { (NSFontAttributeName, NSForegroundColorAttributeName) };
    send!(attrs,"setObject:forKey:",();font,Id;font_key,Id);
    send!(attrs,"setObject:forKey:",();color(tint),Id;color_key,Id);
    let glyph = super::icon_codes::glyph(name).to_string();
    send!(string(&glyph),"drawAtPoint:withAttributes:",();Point{x,y},Point;attrs,Id);
}
pub(super) fn round(r: Rect, radius: f64, tint: [f64; 3]) {
    send!(color(tint), "setFill", ());
    let path = send!(class(c"NSBezierPath"),"bezierPathWithRoundedRect:xRadius:yRadius:",Id;r,Rect;radius,f64;radius,f64);
    send!(path, "fill", ());
}
pub(super) fn outline(r: Rect, radius: f64, width: f64, tint: [f64; 3]) {
    send!(color(tint), "setStroke", ());
    let path = send!(class(c"NSBezierPath"),"bezierPathWithRoundedRect:xRadius:yRadius:",Id;r,Rect;radius,f64;radius,f64);
    send!(path,"setLineWidth:",();width,f64);
    send!(path, "stroke", ());
}
pub(super) fn line(a: Point, b: Point, width: f64, tint: [f64; 3]) {
    send!(color(tint), "setStroke", ());
    let path = send!(class(c"NSBezierPath"), "bezierPath", Id);
    send!(path,"moveToPoint:",();a,Point);
    send!(path,"lineToPoint:",();b,Point);
    send!(path,"setLineWidth:",();width,f64);
    send!(path,"setLineCapStyle:",();1usize,usize);
    send!(path, "stroke", ());
}
pub(super) fn circle(x: f64, y: f64, radius: f64, fill_color: [f64; 3]) {
    send!(color(fill_color), "setFill", ());
    send!(
        send!(class(c"NSBezierPath"),"bezierPathWithOvalInRect:",Id;rect(x-radius,y-radius,radius*2.,radius*2.),Rect),
        "fill",
        ()
    );
}
pub(super) fn knob(x: f64, y: f64, radius: f64, value: f64, active: bool) {
    circle(x, y, radius + 1., [0.16, 0.16, 0.16]);
    circle(x, y, radius - 2., [0.25, 0.25, 0.25]);
    for i in 0..32 {
        let a = (145. + i as f64 * 8.).to_radians();
        let b = (145. + (i + 1) as f64 * 8.).to_radians();
        line(
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
                GOLD
            } else {
                [0.29, 0.29, 0.29]
            },
        );
    }
    let angle = (145. + value.clamp(0., 1.) * 256.).to_radians();
    line(
        Point { x, y },
        Point {
            x: x + (radius - 5.) * angle.cos(),
            y: y + (radius - 5.) * angle.sin(),
        },
        1.4,
        TEXT,
    );
}
pub(super) fn segments(text: &str, x: f64, y: f64, width: f64, height: f64) {
    let mut xx = x;
    let masks = [
        0b1111110, 0b0110000, 0b1101101, 0b1111001, 0b0110011, 0b1011011, 0b1011111, 0b1110000,
        0b1111111, 0b1111011,
    ];
    for c in text.chars() {
        if c == ':' {
            circle(xx + 2., y + height * 0.3, 1., GOLD);
            circle(xx + 2., y + height * 0.73, 1., GOLD);
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
                line(
                    Point {
                        x: xx + a.0,
                        y: y + a.1,
                    },
                    Point {
                        x: xx + b.0,
                        y: y + b.1,
                    },
                    1.4,
                    GOLD,
                );
            }
        }
        xx += width + 3.;
    }
}

pub(super) struct Control {
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
pub(super) fn controls() -> Vec<Control> {
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
