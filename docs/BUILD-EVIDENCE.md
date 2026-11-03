# Local evidence

This ledger retains historical checks and records the v1 release scope separately from full reference parity. Current capabilities and further work are in README.md, CONTRIBUTING.md and PARITY.md. Historical failures and earlier executable hashes remain attached to their original evidence.

## Native CI follow-up · 2026-11-03

[Run 41573613175](https://github.com/PelayoGarrido/Garrido-Studio/actions/runs/41573613175), source revision `deccfa828455fec9c1ea96dd4340d86551bff8e2`, passed all three native desktop jobs: macOS arm64, Windows x64 and Ubuntu 24.04 x64. Each job completed core and editor checks, optimized builds, all fourteen included-plugin diagnostics and archive readback. These checks do not establish real recording-device or long-session musical acceptance.

That run did not publish. Its security job found a yanked-package warning with no advisory object, exposing a parser error. A direct, bounded check of all 401 locked registry records confirmed async-executor 1.14.0 was the sole yanked version. Its official 1.14.1 crate and checksum were verified; the changelog fixes a use-after-free during executor destruction. The lock and license inventory now select that correction. The audit wrapper preserves the raw result before interpreting it and refuses incomplete registry diagnostics. Sixteen Python security/release tests pass, including null warnings and a clean-looking advisory result with a failed registry check. The final corrected package set requires a new complete native CI run.

Initial native setup failures were also corrected: audited assets and license texts retain their exact bytes on Windows checkout, and build scripts resolve Cargo from the configured toolchain path rather than assuming its home-directory location. No account state, original media or approved branding was changed.

## V1 release preparation · 2026-11-03

Version 1.0.0 covers the implemented local workstation flows. Full reference parity remains the contribution roadmap. The final local checks pass **129 core/plugin tests**, **32 optimized common-desktop tests** and **13 Python security/publication regression checks**. Rust formatting and strict Clippy pass. The registry dependency graph is unchanged; owned packages are version 1.0.0.

An initial recording-test run failed because two parallel tests could select the same temporary directory from a coarse timestamp. The helper now claims its own directory with a per-process atomic sequence before any test writes or deletes files. The corrected full run passes. No real recording or user project was involved.

Windows review found a mismatch between ordinary drive paths and canonical verbatim paths passed to a plugin child. The package boundary now recognizes both forms of the application's directory, canonicalizes only that known root before probing the selected path and still refuses traversal/outside-package files. A regression exercises both executable-path forms without loading a plugin or applying resource limits. Native Windows execution remains separate evidence. Packaging explicitly decodes Rust's version output as UTF-8 and records archive member names with forward slashes for portable Windows ZIP verification.

The first release-mode Mac packaging attempt failed because a generated text file was placed in the .app root, which codesign refuses. Release instructions now sit beside the app and the new package passes deep/strict ad-hoc signature verification. Earlier failed staging is retained as a regenerable intermediate.

The actual v1 UI opened the original generated example (six channels, six patterns, six clips) and saved a new copy. The saved native bytes differ through explicit note-channel binding; independent CLI renders are **byte exact**, 486,509 frames, SHA256 `7777436c18c8c36a1565c021bb1434d7678200f015c934cd99235afb0088b262`. This UI result belongs to executable `c9f5f431522afb2613ea8e51355a0a2dd294c6c5b6c2ced28e9c0da7e9b1a73b` before the final host path correction. The final packaged Mac executable is `71bcd67597c7f22f53ba0287c875ee643b5c7111848ae209151556f4bf9badf3`; its public offline diagnostic passes all fourteen included plugins. No musical signal or microphone was opened.

The fresh official RustSec snapshot contains 1,382 records, revision `263a4d951a0867d4bddc29dddd6d59427c78f9a7`, archive SHA256 `eb03b97ab7d77d07bcd1d8287d388de6ef1356ddf866565ea175593c1a2ef228`. It reports zero known vulnerabilities and the same three maintenance warnings. The raw strict scanner remains failed for warnings; a separate exact-artifact review expires 2026-11-17. New vulnerabilities, unrelated warnings, expired reviews and changed artifact checksums remain blocking. The policy tests exercise those cases.

The release workflow checks the version tag on native macOS arm64, Windows x64 and Ubuntu 24.04 runners, including the bundled-module diagnostic. A release publisher requires all three native package reports to match the source commit and version, verifies uploaded GitHub asset sizes/digests and publishes only after every required file is present. This describes the configured gate; it is not a claim that the remote workflow has already run.

The old dated preview hashes and measurements below remain historical evidence. The approved logo/moodboard are unchanged. Credentials, account state, source, original media and earlier archives are preserved.

## Historical verification · 2026-10-31

`pnpm check` passes **127 core/plugin tests**, formatting, strict Clippy and the four-package/zero-registry core audit. `pnpm desktop:check --reuse --release` passes **32 optimized common-desktop tests** and strict Clippy; the same 32 tests also passed in the earlier debug check. The final root/common runs therefore total 159 passed test executions; the earlier debug run checks the same common suite. Both actual cross targets pass all-target strict Clippy for the core, plugins and common GUI through `pnpm qa:lint:windows` / `pnpm qa:lint:linux`. Logs and count summary: `outputs/*-closure-*check.log`, `outputs/*-closure-clippy.log`, `outputs/local-closure-checks-20261031.json`.

New tested behavior includes bounded persistent theme/audio/ghost-note preferences and recent files; named, seekable history; own AIFF/AIFF-C input and AIFF output; aligned active-channel stems; multitrack disk recording with retained originals and recovery; and a long-song block compiler separate from whole-song offline rendering. Synthetic capture covers selected/odd-mono layouts and all sixteen stereo input pairs without opening a device. Simultaneous take insertion is one undo transaction and failure preserves every pending sample. Allocation instrumentation covers processing, queue transfer and capture publication. Long-song tests process fixed blocks and reject full-buffer rendering before allocation. Actual microphone permission, alignment, unplug and power-loss behavior remain pending.

The current release CLI is `98e17b78bd76886c95220bc185d67f67d7197749b52aa00fe23b5772ccf08b33`. `pnpm check:audio` verifies the owned 12,345-frame stereo/48 kHz/24-bit fixture against Apple's `afconvert`: own FLAC and AIFF decode to exact PCM in Apple tools; Apple FLAC, AIFF and integer/float32/float64 AIFF-C decode to the original PCM in our reader. Independent hashlib agrees with the FLAC PCM MD5. These are fixture-specific results; no listening acceptance or universal format compatibility is claimed.

The new common Mac package is staged separately, preserving earlier applications and the previously open session. Executable SHA256: `0b6faf0e2a7bb2fb4678ea9feb7a56afa012910a8fb5c98dd13046f3c6430604`. Its ad-hoc signature verifies, including the plugin. All 14 first-party worker checks pass, with maximum Gain error `5.4076668698499475e-09`. A complete relocated package also passes; modifying only its copied plugin is rejected. The public diagnostic creates both PASS and FAIL reports as appropriate and preserves the earlier report. Source plugin bytes remain unchanged. The packaged zero-signal output probe consumed **44,544 frames, zero underruns**, peak/RMS zero and a healthy producer. No microphone or audible signal was opened. Reports: `outputs/desktop-plugin-smoke-closure-20261031.json`, `outputs/macos-closure-stream-probe.json`. Earlier load-related probe failure remains recorded below; this new short run is not stress or physical latency acceptance.

The common CLAP host now has native loaders/resource controls for all three systems and requires the exact packaged first-party module and SHA256 manifest. Windows uses System32 dependency lookup and a process job; Linux uses resource limits and no_new_privs. These controls are not accepted sandboxes for foreign code. External modules are rejected on every platform. Windows/Linux native runtime execution remains pending; the Mac-only worker tests do not validate their system calls.

Current application archives, read back against every staged file:

| Target | Executable SHA256 | Archive bytes | Archive SHA256 |
|---|---|---:|---|
| macOS arm64 | `0b6faf0e2a7bb2fb4678ea9feb7a56afa012910a8fb5c98dd13046f3c6430604` | 12,165,524 | `8150b8dd5625053e44306d170cc514dcbfa591ad87ab250352bcc2b6641eb8df` |
| Windows x64 | `1f14e858be2eef01208278158f2b43c7268554a466b6689f67d2aa2f156ed4c5` | 9,575,515 | `c6c0b44762ff051fdc83f551709bd6bf71bc9f5a536d6905d7b571ac23cde696` |
| Linux x86_64/glibc 2.39 | `b0b5d8df350018584615302aae79b1f26ff42a41303f907a4fbb5b78f12d9055` | 10,143,356 | `368c977d5031cdd065278decab9df3dc3411c502082a4c1a339b0e6aacbb2796` |

Archive names are `dist/desktop/Garrido-Studio-<target>-Preview-20261031.*`; each adjacent JSON lists every file and hash. Windows PE inspection verifies ASLR/NX/high-entropy VA and the GUI subsystem; Linux ELF inspection verifies NX/PIE/full RELRO, no embedded runtime search paths and the glibc 2.39 baseline. These are static inspections. Linux linkage still emits a deprecated linker-optimization-option warning, distinct from the successful strict Clippy checks. App packages include the synthetic session, notices, manual acceptance guide and an offline first-party-plugin diagnostic launcher on Windows/Linux. No foreign binary was executed on this Mac.

The official RustSec refresh uses revision `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, 1,375 advisory records, archive SHA256 `bb962bdfe1ed50e91a82c819acaa40865b6c35e380856f3cd751d71f95b72dd3`. The unchanged 401-package desktop lock has zero known vulnerabilities and the same three unsuppressed maintenance warnings; the advisory gate remains failed. All 401 dependency license texts are located. No dependencies were added for the new features. The approved logo/moodboard and all seven brand hashes remain unchanged; dark remains the initial default and light requires explicit selection.

Both updated offline QA kits contain **22 compiled test programs**, a launcher, matching application/lock fingerprints and notices. Every Windows test executable was inspected as PE32+ with the console subsystem; every Linux program passes ELF hardening/path inspection. Linux shell launchers pass syntax checks. No target executable was run here. Archives were read back against all staged files:

| QA kit | Bytes | SHA256 |
|---|---:|---|
| `Garrido-Studio-Windows-x64-Checks-20261031.zip` | 17,063,736 | `884de02fa41b962815d7a5cf5e534bf5b6b75e6081d3713829b483269855b831` |
| `Garrido-Studio-Linux-x64-Checks-20261031.tar.gz` | 14,375,836 | `0c2074958c97cd8b59f09848c578ef00cb549b81ec3a9a8a72421ec2e856c964` |

The CI packaging step now includes the native first-party-plugin diagnostic. The same packaging option passed locally in a separate Mac variant with the same executable fingerprint; the cross-package/execute combination was explicitly rejected before staging. The GitHub workflow itself has not run. A bounded source scan checked 852 files for selected credential/private-key patterns and found no matches, without printing file contents; this is not exhaustive secret detection.

Physical free space was monitored on `/System/Volumes/Data`. Only regenerable project caches (`target/windows-check` and `target/desktop/debug`) were removed, after retaining test logs; source, toolchains, packaged apps, creative originals and prior archives were preserved. Cached checks/builds retain a 3.5 GiB start threshold and a 3 GiB active abort reserve. No commit, remote, upload or release was created. **The full v1 remains incomplete**, including FLP coverage, live/external plugin workflows, MIDI/live editing, long file-backed sessions/exports, other matrix items and real platform/device acceptance.

## Historical checks and package · 2026-10-28

- Host: Apple Silicon arm64, macOS 15.8.1. Pinned official Rust 1.99.1; builds use offline/locked Cargo and two jobs.
- `pnpm run check`: passed formatting, all-target Clippy with warnings denied, **85 tests** and dependency/font/brand audit. No ignored/failed tests in the final log (`outputs/core-channel-check.log`).
- Test groups: six own CLAP, four native capture fixtures, 32 core, eight channel/migration, four export/edit, six FLAC, one digest-vector, thirteen FLP, four notes, one realtime allocation, two recovery and four routing tests.
- The allocation test observes **zero allocations during engine block processing**, including routing and the optional metronome. Apple driver allocations and graph construction are outside that measurement.
- Core audit: four own Rust packages, zero Cargo registry dependencies/external build scripts; four font/license and seven approved-brand SHA256 values verified. This is not an exhaustive vulnerability or unknown-exploit clearance. Current advisory findings and unresolved limits are in SECURITY.md.
- Historical native `pnpm run bundle`: passed; packaged arm64 application and own CLAP bundle have verified ad-hoc signatures. This is not Developer ID signing, notarization or clean-machine acceptance.
- Historical native executable SHA256: `4b4ad4594a77cc04f81e6994a1a36623fe40dd0dd5986559106a5c7572bd041a`.
- Executable: 1,085,440 bytes. Total application files: 5,039,275 bytes, including own CLAP, upstream fonts/licenses and standard-library notices. See `outputs/build-reference-report.json`.
- Runtime links are Apple-supplied AudioToolbox, AppKit, Foundation, CoreText, CoreFoundation, Objective-C, libSystem and libsandbox. No web runtime or cloud service.

## Common desktop, 2026-10-28

The first-release requirement includes macOS, Windows and Linux. `apps/desktop` supplies the shared Rust GUI with its own lockfile. On this Mac, strict Clippy and 22 silent/headless tests cover callback bounds/pair selection, audio preferences, canvas persistence/undo/redo, drag transactions, rapid click/modifier timing, menu isolation, file opening/migration, rectangle selection, shared channel patterns, clip resizing and explicit dark/light selection with text/control contrast. The original core/native workspace still passes its own checks. Strict Windows cross-target Clippy covers core/plugins/common GUI, and Linux cross-target Clippy covers core/plugins. Neither links or executes those applications. Linux GUI checks and all non-Mac runtime acceptance remain pending; the three-platform workflow has not run.

Actual optimized-app checks opened the workspace, added Trio, entered a B4 note at tick 960/length 240/velocity approximately 0.65, saved/reopened it, selected its pattern, placed a clip on playlist row 2 and saved a new copy. The first fixture intentionally had no playlist clips; a song export of that empty arrangement contained only dither. The arranged fixture's WAV was byte-exact against a separate CLI render: 450,462 stereo frames, 48 kHz/24 bit, 2,702,816 bytes, peak 0.06991315 and RMS 0.00380784. Reports: `outputs/ui-desktop-smoke-report.json` and `outputs/ui-desktop-export-report.json`. The source fixture was not overwritten.

A pre-brand common-output probe received 44,032 frames of zeros. It opened no microphone and changed no system volume. The common bundle's worker scanned 14 own descriptors and rendered eight effects / six instruments, with gain maximum absolute error 5.4076668698499475e-9; an altered copy was refused before loading. Report: `outputs/desktop-plugin-smoke.json`. This is first-party worker evidence, not third-party DAW/plugin acceptance.

The common dependency graph has 401 registry packages including non-shipping target dependencies. Official RustSec cargo-audit 0.22.2 found zero known vulnerabilities and three unsuppressed Linux maintenance advisories, so the advisory gate fails. `legal/desktop-dependencies/inventory.json` records a located notice collection for every registry package; notice inventory does not certify legal clearance. See DEPENDENCIES.md for source-revision caveats and the owned GCD compatibility patch.

The internal common bundle is under `dist/desktop/macos-arm64/`, with its own `build.json` fingerprint. It remains an internal prealpha and publication gate FAIL. The native comparison fingerprint above is historical evidence. Rebuild and recheck the common bundle after source changes; source-test results and a prior binary fingerprint are separate evidence.

## Historical native UI

The previous approximate UI was replaced with an independently measured reference layout. Actual app captures were inspected against both supplied screenshots: compact transport/tools, 205px browser, pattern picker, 48px playlist rows, floating rack and piano, fixed piano zoom/key geometry and upward velocity lanes. See UI-REFERENCE.md for coordinates, retained differences and asset provenance. Fonts/icons come from upstream OFL/MIT distributions; no reference source is ported. Exact pixel/glyph identity is not claimed.

Native checks placed C4/E4/G4/C5 at ticks 240/1200/2160/3120, saved a new session, moved the first note to tick 720/key 61, changed the second velocity to approximately 0.8993 and saved a second copy. An independent Python read checked native CRC and exact note values (`outputs/ui-reference-report.json`). Undo, ADD selection, New session and eraser/undo were inspected. Piano maximize/restore and application maximize/restore worked. The custom close button showed the unsaved-work dialog, cancel retained the note, and closing after undo terminated normally.

Controls have native accessible names and keyboard actions. Canvas editing still lacks complete screen-reader support. Native menus/forms, own branding, Cuprum 1.002 versus the reference's older 1.001 and native rasterization remain visual differences. Other reference editors/workflows are not accepted by these screenshots.

## Audio and interoperability

- `pnpm run check:plugins`: fourteen descriptors, eight effects and six instruments rendered through the signed first-party bundle. Gain maximum absolute error: 1.668930053738693e-7. Modified bundle rejected. No third-party binary was loaded and no audio was played. Report: `outputs/plugin-smoke.json`. The plugin's final SHA256 remains `490fa9b59dc381ba2864413cac29166fff6e54f1b073f2ae91f01e6c6ed5c560`.
- `pnpm run check:audio`: own FLAC decoded by Apple and Apple FLAC decoded by our implementation, both **PCM-exact** for the generated 12,345-frame stereo/48 kHz/24-bit fixture. Own FLAC 30,103 bytes versus 74,070 PCM bytes; Apple FLAC 29,615 bytes. This proves that fixture, not every possible FLAC file. CRC8/16 and advertised PCM MD5 are checked. The own checksum matches independent Python hashlib; Apple supplied a matching PCM checksum. Published digest vectors and RFC 9639 decoding example 3 also pass. Report: `outputs/audio-smoke.json`.
- FLP GUI fixture: imported notes, edited 120 to 121 BPM and saved a new FLP copy. Independent event parsing found all non-tempo events unchanged and the source hash preserved (`outputs/ui-flp-report.json`). The saved native session decoded successfully through the CLI. Two pinned public real-file fixtures now import and round-trip byte exactly without edits (`outputs/flp-corpus-report.json`); there is **no FL Studio application acceptance**; original instruments are substituted and playlist/mixer/automation playback is not interpreted.
- Recovery copies and the restore menu were observed; full quota correctly pauses recovery without deleting earlier copies. Actual restoration/crash/power-loss acceptance remains pending.
- Earlier zero-signal AudioQueue probe observed callbacks and synchronous closure; no musical playback was used. Recording tests use owned callback buffers. No microphone or permission acceptance was performed.
- Source test WAV remained unchanged through import/edit/undo/save: SHA256 `7777436c18c8c36a1565c021bb1434d7678200f015c934cd99235afb0088b262`.

## Offline performance

`pnpm run bench`, release build, unchanged fixed six-track generated demo at 48 kHz (`outputs/bench-current.txt`):

```
audio_s=9.136 render_s=0.115647 realtime_factor=79.00
peak_voices=9 stolen=0 peak=0.352206 rms=0.056784
```

Construction, rendering and statistics are timed; compilation, process startup and file export are excluded. Other smoke checks were active during this run. This small offline workload does not establish physical latency, dropout resistance, thermal behavior or a worst-case execution bound.

## Storage and acceptance

Recent physical measurement: approximately 9.3 GiB free on `/System/Volumes/Data`; unrelated activity can change this value. Core build/packaging/recovery retain 3 GiB; the common GUI requires 5 GiB before a build and cross-target checks require 6 GiB before starting. No preexisting creative work, sessions, plugin installations or system assets were cleaned. Disposable smoke fixtures were limited to directories created by those tests.

Full FLP arrangement/plugin/automation semantics, arbitrary third-party hosting, VST, real recording devices/permissions, CoreMIDI/sync, streaming/live editing, reference tool completeness, advanced routing, lossy formats/stems, linked Windows/Linux apps and runtime acceptance, accessibility, musical listening, signing/notarization and legal/distribution clearance remain pending. Passing local checks does not satisfy the owner's full-feature publication condition.

## Version 4 channel model and approved brand

The current codec reads versions 1–4 and writes version 4 with checked per-note channel/source IDs and playlist row mutes. Migration tests preserve the rendered audio of legacy shared patterns and never modify the input project. Unarranged legacy channel assignments are inferred as documented in ARCHITECTURE.md. Engine/MIDI work budgets reject excessive arrangements and skip empty one-tick patterns.

An actual shared-window test added Trio and Bass to one pattern, entered B4 at tick 960 and A4 at tick 1680 (each length 240, velocity approximately 0.65), placed the pattern on playlist row 3 and resized its clip to 11,520 ticks. The saved version 4 session has three channels, one pattern, two independently assigned notes, one clip and a valid CRC. Independent Python parsing and the release CLI confirmed the result. Source: `outputs/ui-common-multichannel-v4.gstudio`, SHA256 `f415946407be24703555728722da4f94caed5f4eea1d7ef86ecd89f952d7d5b6`; report `outputs/ui-common-v4-report.json`. A copy is retained as the owned manual-QA fixture. No audio was played.

The owner supplied the official logo and approved the generated brand board. Both originals remain byte-identical with recorded hashes. The common app now uses this identity, starts in dark mode even on a light system preference and offers explicit light-mode selection. Actual initial-dark and opt-in-light screenshots were inspected; subsequent visual refinements retain the same layout. Theme changes leave musical state and dirty status untouched in tests. The current raster moodboard is conceptual and is never used as runtime evidence. See BRAND.md and the local bundle build.json for the current executable fingerprint.

Audio preferences select input/output devices, rate, buffer size and a stereo pair from up to 32 input channels. The real device list was inspected without opening input. Pair selection/capture limits are tested with owned buffers; no microphone, actual recording permission or physical latency acceptance is claimed. The owner confirmed access to Windows and Linux for later runtime tests; PLATFORM-ACCEPTANCE.md records the prepared checks.

## Final branded shared-binary export check

Common bundle SHA256 `333e13129faa5c465dfa30b6c2a172e10e746b66087e539b01d8626ae9f10c04`; total staged files 24,366,321 bytes. The current signed first-party worker smoke report identifies the same executable. The actual GUI exported the owned multichannel fixture to a new WAV; it is byte-exact against a separate release-CLI render: 450,462 frames, stereo 48 kHz/24 bit, 2,702,816 bytes, peak 0.10529828, RMS 0.01290364. SHA256 `beabdc545067c31a1c737a652f0ccc1c9e5f3ba2d865dbbdfa8370d259ca92b3`. Report: `outputs/ui-brand-export-report.json`. The fixture hash remained unchanged. No audible playback or microphone was used.

Dark startup, explicit light selection and return to dark were inspected in the actual common application. The final dark capture is `outputs/ui-brand-dark-final.png`; earlier opt-in-light captures remain local evidence. The approved logo and moodboard hashes are unchanged. Packaging rejects linked parent directories and linked output files; a bounded temporary-file check confirmed refusal without modifying their target files.

Final visual-only refinement: themed popup backgrounds, dark selection text on coral, readable knobs/scroll handles and rounded presentation of the unchanged official mark. The staged executable is `07843b7dbfde06a4a2bf60e9ac8df909982e24e79b1f48225562e799f829a57e`; 22 shared tests and strict Windows-target Clippy pass for this source. First-party worker checks identify this final executable. The preceding GUI/CLI export result belongs to its recorded binary and the unchanged audio core; it is not relabeled as a new export. Final light capture: `outputs/ui-brand-light-final.png`.

The final staged app was reopened after the popup contrast fix. Actual dark startup, explicit light selection and restoration to dark were inspected again; the dark default capture is `outputs/ui-brand-dark-default-final.png`. The approved board/original logo still pass all seven brand hashes. The latest silent-output probe received 44,544 frames; no input was opened.

The final packaged executable also passed its own one-second zero-signal output probe: 43008 frames, fingerprint `07843b7dbfde06a4a2bf60e9ac8df909982e24e79b1f48225562e799f829a57e`; report `outputs/desktop-final-probe.json`. This tests output callbacks only and opens no microphone.


## 2026-10-29: audio arrangement, FLP playlist and block playback

The core/workspace now passes 101 tests and strict Clippy; the common desktop passes 25 headless/silent tests and strict Clippy. New tests cover phase-exact audio cuts at fractional resampling positions, full 64-voice cuts without stealing, compatible joins, mute/group/clipboard/undo, source position across tempo changes, format-5 persistence and the unchanged version-4 fixture. A concurrent variable-block queue transfer equals an offline render exactly. Allocation instrumentation observes zero allocations in both engine processing and 1,000 queue transfers. These are bounded local tests.

The selected FL 12.9.1–20 playlist reader interprets validated uncropped pattern records. The public FL 20.8.4 fixture now imports five placements, with original musical position, duration, rows and groups. Seventeen unsupported channel/cropped/flagged records remain opaque and reported; one placed pattern has no importable MIDI notes. The second real fixture has an explicitly empty selected playlist and now correctly places zero clips. Both unedited FLP exports remain byte exact; `outputs/flp-playlist-corpus-report.json` binds this to CLI `96c363beada9514bf2354b0ae1b4866a72bef2485936ac5aaa8f9bda4b56d14e`. No FL Studio application acceptance is claimed. Format research used the pinned upstream data descriptions in [PyFLP arrangement.py](https://github.com/demberto/PyFLP/blob/f937126b888ce94271bfea631b89166c74056530/pyflp/arrangement.py); no reference implementation is incorporated.

An independently encoded four-second synthetic session was opened in the actual common Mac app, cut with Alt-click, saved through its native dialog and joined through EDIT. The original and GUI-cut saved sessions render to identical WAV bytes: SHA256 `17930ba8f1036a3f167607ca2d127732d9103e8861c8c53dc5d4e2d21b37be43`, 450,462 frames. The report is `outputs/audio-arrangement-ui-report.json`; real captures are `outputs/ui-audio-clips-split.png` and `outputs/ui-audio-clips-joined.png`. This UI evidence belongs to executable `822501e763d29a61743b01b4ea76ee831673e106c270956f160cdfa03b13f9cb` before block playback integration.

The optimized new block-playback binary passed a real one-second **zero-signal** output probe: 43,008 frames and zero underruns. No microphone or audible musical playback was opened. Its device timing, musical listening quality and Windows/Linux runtime behavior remain unaccepted. The fixed demo offline benchmark was 71.21 times real time under this session's concurrent workload, with nine peak voices and no stealing; this is not a physical latency or worst-case bound.

RustSec was refreshed from official revision `40f5be6fb12ed93fc591bf4ab23b7d793b3f2239` (1,361 records). The unchanged 401-package lock still has zero known vulnerabilities and three unsuppressed maintenance warnings. Report timestamp is recorded in UTC; the local date is 2026-10-29. The newly added pulldown-cmark advisory is outside this lock's affected dependency set. The audit gate remains failed.


The separately staged block-playback app is `dist/desktop/macos-arm64-preview-20261029/Garrido Studio.app`, executable SHA256 `f94f2a6ae4461127794a3e1497beb887c72824cec4488db55183919e285cfd55`, 24,459,762 bundled bytes. Its ad-hoc signature verifies. Its packaged zero-signal streaming probe consumed 43,520 frames with zero underruns (`outputs/desktop-stream-probe-20261029.json`). All 14 first-party plugin worker checks pass for this exact executable, including altered-bundle refusal (`outputs/desktop-plugin-smoke-preview-20261029.json`). The older GUI session was left open because new edits appeared during validation; its content was not undone, closed or overwritten. Both app variants remain internal previews.


A probe of the subsequent Mac executable `29fd14f27c73060fbbfb1ab744ade2007b6778f3ad5244f86939ee282716f4e9` failed its interruption check while concurrent cross-link work was active. That attempt did not expose detailed counters in its error. A separate run of that exact executable then passed with 44,032 silent frames and zero underruns (`outputs/desktop-stream-diagnostic-20261029.json`). This is not treated as a clean stress result. The callback now explicitly distinguishes pre-play startup from active consumption, with a separate test, and the probe emits counters even on failure; this change alone does not establish the earlier failure's cause. Common tests now total 26.


The startup-state correction is packaged in the current separate Mac preview: executable `a691dab4c17c1f5e15b96e37f362f3cef9c8c3b4a513993a734382142efb5354`, 24,460,115 bundled bytes. Its 14 first-party worker checks still pass. Probe counters are saved under `outputs/desktop-stream-probe-20261029-final.json`; earlier executable fingerprints remain attached to their own earlier results.


The complete Windows x64 GUI/core/plugin cross build now succeeds with the unmodified official compiler runtime linked explicitly. GUI PE32+ AMD64 fingerprint: `9b0fde6b05b54936f09f471cfec7f2718176d9905b65c8cd6ecada4b6530a708`, 14,964,224 bytes. PE inspection reports ASLR, NX and high-entropy VA, with system/API-set DLL imports; it is a static inspection, not a Windows run. The staged ZIP is 9,415,347 bytes, SHA256 `e41ad993dc49c3b3819b242be766f94f443649633dcd635251874f3a4628af71`. All 717 archived files were CRC-checked and compared against the staged bytes. No Windows runtime acceptance, third-party plugin hosting or signature is claimed. The archive includes a generated-source test session, notices and the manual acceptance guide. The owner has been asked for actual Windows test results.


The complete Linux GUI/core/plugin cross build also succeeds (7m07s for the cold GUI build with one job). GUI fingerprint `c38c20a089acdb07eb84ef0029e0da930bb692c9f5af7a83de0aab1a460aa048`, 20,678,616 bytes. ELF inspection confirms NX, PIE, full RELRO and no Mac/sysroot runtime search path. Its imported glibc versions reach GLIBC_2.39; supported testing baseline is Linux x86_64/Ubuntu 24.04 or equivalent, with actual desktop acceptance pending. The tar archive is 10,031,393 bytes, SHA256 `673e3fada755269cf012e1e9da6fb6c11523857b4e563bb8ab8c1af01a97aeca`; all 717 files and executable modes were checked against the staging directory. Build report: `outputs/linux-build-report.json`; ELF report: `outputs/linux-elf-inspection.json`. No Linux binary was executed on this Mac.


Current delivery validation: **101 root/workspace tests and 26 common-desktop tests**, with strict Clippy. Complete cross-linked Windows and Linux GUI/core/plugin outputs are packaged for owner testing; their execution remains pending. The approved moodboard and official mark are unchanged, the app defaults to dark, and the previously open GUI session was left intact. Physical free space stayed above the 3 GiB abort reserve; the last packaging checks were around 4.4 GiB free. Full reference functionality, external acceptance and publication remain unfinished. No Git commit, remote or upload has been made.


## Target-system validation handoff

The current separate Mac preview executable is `698677468e1493ce90076940c99b3f9393d0886a5668b8253c3bc9616527f8ce`. Its zero-signal probe passed with 44,032 consumed frames, zero underruns and a healthy producer while Windows cross-test compilation was active (`outputs/desktop-stream-current-20261029.json`). The current first-party plugin report also identifies this executable: all 14 descriptors pass, modified bundles are rejected, maximum gain error is `5.4076668698499475e-09`, and no microphone, audible playback or third-party code is opened. Earlier probe failure and stress limitations remain recorded above.

Source review found that Windows recovery traversed a drive prefix before its root. This has been corrected, with an additional Windows-only regression test. The existing 101 Mac core/workspace tests and strict Clippy pass after the correction; the new Windows test is compiled, with actual execution pending. The shared Mac GUI previously passed its 26 tests; its new conditional subsystem declaration affects Windows test programs only. The corrected Windows GUI/core/plugin link successfully, and all 17 Windows test executables have ASLR/NX and the console subsystem in static PE inspection. The application retains the GUI subsystem. Reports: `outputs/windows-build-report.json`, `outputs/windows-pe-inspection.json`, `outputs/windows-qa-pe-inspection.json`.

Current checked app archives:

| Target | Bytes | SHA256 | Target runtime |
|---|---:|---|---|
| Windows x64 | 9,451,555 | `b133fd238245378c3f4506a012ade3fb8ded93334041d8574b0ce3b10c77c347` | Pending |
| Linux x86_64/glibc 2.39 | 10,035,248 | `36068ce95ba58e10ec41a8ee9cb9a6209d4d438de3a3463da9f6cf9d925d15ff` | Pending |

Both archives were read back against every staged file. Offline test kits contain 17 compiled executables per target, their own/dependency notices and a launcher producing `Results.txt`; they need no development tools and use no audio devices. Their manifest names the matching application and desktop lockfile. The owner will provide actual Windows and Linux results. Neither a compiled test suite nor successful target execution alone would complete the remaining reference functionality or the v1 publication gate.

Final offline test-kit archive records (compiled, not executed on target):

- `Garrido-Studio-Windows-x64-Checks-20261029-2.zip`: 14,382,093 bytes, SHA256 `28165180f455e59a3c2eaf2f202d705a69c056bc6d752cccc88b1c96e317551d`, all 727 archived files read back successfully.
- `Garrido-Studio-Linux-x64-Checks-20261029-2.tar.gz`: 12,048,533 bytes, SHA256 `bb6898a935452c6b00d1a81b6b99a4732b524b262e827c15084e8288b9363d44`, all 727 archived files read back successfully.

Test launchers include the application fingerprint and system identification. Linux launcher syntax and all 17 ELF test programs were checked without executing them; Windows test programs were checked as PE32+ console applications. The supplied notices cover the embedded dependencies/fonts. The earlier Windows check archive without notices is superseded by `-2`. No test result from Windows or Linux has been received.
