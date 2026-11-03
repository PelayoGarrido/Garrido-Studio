# Workspace evolution

Version 1.1 replaces the reconstructed transport presentation with Garrido Studio’s own header, grouped playback controls, named view tabs and native window chrome. The library uses three labelled functional tabs; the wider step sequencer shares its proportional geometry between drawing and hit-testing. Menu anchors follow the actual control positions. The approved logo and palette remain unchanged.

The record below describes the earlier comparison work. It is retained for provenance and is not the current design specification or a legal clearance.

# Reference UI reconstruction · 2026-10-28

Target: the owner's two supplied Lemon Studio screenshots, from revision `a6ce2dd443d75abf424a50fb5dd68526f87bed9d`. The previous merely similar workspace was replaced with independently written Rust/AppKit drawing and gestures. The application starts with an empty sampler/pattern, matching the reference's initial workspace. Branding remains Garrido Studio.

The owner subsequently supplied the official logo and approved a coral/rose/violet moodboard. The common app retains the measured structure while applying that identity. Dark mode is the startup default; light requires explicit selection. The approved moodboard remains unchanged. See [BRAND.md](BRAND.md). Earlier neutral/yellow native captures below are historical layout evidence, not the current brand palette.

## Measured layout

Coordinates are design pixels at 1920×1080. Native window width determines the scale; the initial content ratio is 16:9.

| Element | Geometry |
|---|---|
| Transport / tools | Two rows, 40px each, separated by 1px |
| Browser | x=0, y=82, width=205; icon row, content tree and TAGS footer |
| Pattern picker | x=209, width=159; 135×29px selected pattern |
| Playlist | Track labels x=369; grid x=481, y=148; 48px tracks and at least 24 bars |
| Floating rack | x=278, y=height−380, size=511×247; 28px channel rows, 16 steps |
| Floating piano | x=width−819, y=176, size=779×719 at the reference size |
| Full piano | x=209, y=82, width=window−209, height=window−82 |
| Piano grid | 18px semitones, 96px per beat initially, short black keys, C octave labels |
| Event lane | Bottom of piano, upward velocity stems, open circles and duration caps |
| Overview | Note preview above the ruler; scroll thumb reflects visible time |

The top transport, toolbar, browser tabs, title icons, separators, rounded borders, grid tones, step buttons and piano keyboard have been reconstructed. Native semantic buttons remain behind the drawn toolbar for keyboard actions, tooltips and accessible names. Full screen-reader editing of the canvas remains pending.

## Fonts and assets

Tabler 3.50.0 stroke-1.5 glyphs come from its official MIT package, with registry integrity verification. Cuprum 1.002 comes from the historical official Google Fonts OFL distribution. Both are unmodified; licenses, source revisions and hashes are retained in `assets/fonts/`. Registration is process-scoped; no fonts are installed in macOS.

The reference uses an older Cuprum 1.001 file. Its metadata was inspected in ignored research output, but that file is not redistributed: its exact distribution provenance was not independently established. Accordingly, font shapes/metrics and native rasterization retain differences. The previous current Cuprum 3.000 file was replaced because it differed more visibly.

## Actual UI checks

- Opened the packaged native app and inspected the default rack/piano layout and maximized piano against the reference images.
- Placed C4/E4/G4/C5 notes, saved a new native project, moved C4 by 480 ticks and one semitone, edited E4 velocity to approximately 0.8993 and saved a second copy. Independently parsed the files and checked their CRC and note values. Evidence: `outputs/ui-reference-report.json`.
- Inspected undo, native save, ADD selection, New session, piano maximize/restore and eraser/undo without audible playback or microphone access.
- Verified application maximize/restore with a changed native window size. Closing with an unsaved note shows the save/cancel/discard dialog; cancel preserved it. After undo, the custom close button terminated normally. Window actions run outside the state borrow so native lifecycle callbacks can check unsaved work.
- `outputs/ui-reference-final.jpg` is an actual application capture; its normalized comparison copy is `outputs/ui-reference-normalized.jpg`. These are ignored local evidence, not images used to render the UI.

This is close reconstruction of the supplied workspace views, not a claim of pixel identity or complete reference UI/workflow parity. Native menus/forms, font version/rasterization, own branding and unavailable reference tools remain differences. Mixer/plugin/settings screens require their own references and implementation. Visual matching does not establish copyright, design-right or trade-dress clearance; see LEGAL.md. Publication remains gated by PARITY.md.

## Shared desktop validation

The same owned drawing and editing logic now runs through eframe/egui/WGPU in `apps/desktop`; AppKit remains a local comparison. Actual shared-window checks covered opening the saved fixture, rapid picker clicks, placement of a pattern clip, Command+S/O, typed WAV export, native dialogs and saved-file integrity. The exported arranged fixture was byte-exact against an independent CLI render, at stereo/48 kHz/24 bit. No audible playback was used.

The shared menus/forms expose semantic widgets. Pointer clicks that start/end between frames, final drag movement on release, per-event shortcut modifiers and double transport activation have dedicated regression coverage. Menus scale with the canvas and keep keyboard selection legible. These checks cover the named flows, not every tool/editor, screen-reader operation or platform.
