# Security boundaries

The application has no cloud backend, network listener, telemetry, updater, credential store, administrator helper or automatic plugin discovery. Imported projects do not instruct it to load libraries, follow URLs or read external samples. Media and session sources remain unchanged. Playback and input capture require explicit controls and never change system volume.

## Inputs and resource controls

| Boundary | Control |
|---|---|
| Native project | Checked lengths/counts, UTF-8, CRC32, finite values, references, routing-cycle rejection and final model validation |
| FLP | 32 MiB/event/note limits, checked varints/chunks, raw opaque preservation, explicit instrument substitution and refusal of unsupported edited exports |
| WAV | RIFF alignment, bounded chunks/channels/rates/depths, finite sample checks; own encoder/decoder |
| AIFF/AIFF-C | FORM/COMM/SSND/FVER validation, bounded chunks and offsets, integer/float formats, finite samples and checked extended-rate decoding; no referenced-file loading |
| FLAC | 64 MiB input / four million decoded frames, bounded blocks/metadata/frame count, Rice quotient and prediction checks, CRC8/16, checked LPC arithmetic and advertised PCM MD5; no linked picture/URI loading. MD5 checks format integrity, not authenticity |
| MIDI | Bounded VLQ/ticks/events/notes/channels; unsupported events reported |
| File input | Regular-file checks, no-follow/nonblocking flags on macOS/Linux, descriptor recheck and bounded reads, including plugin metadata/manifest files |
| File output | Mode 0600 temporary file, file sync and no-replace hard link; an existing destination or symlink cannot be overwritten. The macOS path does not open a directory for sync, which caused a native save/quit hang |
| Recovery | Private application directory, no symlinked folders/files, eight-copy / 128 MiB quota and UI free-space check preserving 3 GiB. No automatic pruning or overwriting. Concurrent-instance quotas are a policy check, not a filesystem-wide atomic reservation |
| Engine | 64 voices, preallocated effects, note/render/work/memory budgets, acyclic graph, bounded sends/internal levels, finite-state cleaning, DC blocker and digital peak ceiling |
| Editing/history | Clone/validate/commit, immutable shared samples/source FLP, bounded undo depth and estimated retained memory; interrupted takes remain available |
| Preferences | Bounded CRC32-checked file, validated device IDs and recent paths, private atomic replacement. Corrupt configuration is retained and blocks automatic overwriting; recent paths never cause startup file loads |
| Workers | Immutable snapshots, cancellation, bounded results and joined shutdown. File encoding/export runs off the UI thread |
| Native adapters | Typed C layouts, explicit lifetimes, guarded UI callbacks, stop flags, callback buffer bounds/alignment and synchronous queue disposal; see FFI.md |
| Bundled fonts | Official upstream Cuprum 1.002 / Tabler 3.50.0 files and licenses, pinned provenance, four SHA256 checks during build/audit and process-only CoreText registration. No font downloads or system font installation at runtime |

CRC and FLAC checksums are integrity checks, not authentication. They do not make untrusted data safe; semantic and resource validation still apply. The limits bound specific allocations/work but are not a formal whole-process memory or worst-case timing proof.

## Native plugin worker

The GUI never loads a native plugin library. Only the exact included `GarridoNative.clap` location inside the current application package is accepted, after lexical/canonical path checks and SHA256 equality with its packaged manifest. Relative paths, parent traversal and copied libraries outside the package are refused. On macOS a bounded XML Info.plist resolves a simple executable name confined to the bundle. The manifest checks content integrity; it is not independent publisher authentication against an attacker who can modify the whole app.

Scan and processing run in a separate invocation of our executable. The worker argument is required before resource limits can be applied; the ordinary host/test process cannot accidentally limit itself. The child starts with an empty inherited environment, except Windows SystemRoot. Pointers stay in that worker; the parent accepts bounded metadata or finite stereo audio only. Input/output pipes have separate threads, a child lifetime guard, cancellation and wall deadlines (4 seconds scan / 12 seconds render).

Before loading plugin code, the child applies a default-deny macOS sandbox. Only the selected bundle/flat file, system library paths, the own executable and required system queries are readable. Network, user-home data, file writes and child creation are not granted. There is no automatic directory scan or installation.

On the tested macOS, both address-space/data `setrlimit` requests fail. The included repository-owned DSP runs under the remaining sandbox/CPU/descriptor restrictions. The sandbox API is deprecated and needs future OS acceptance or replacement.

Linux lowers CPU/address-space/file-size/core/descriptor limits and sets `no_new_privs` before `dlopen`. Windows uses an absolute library path with System32-only dependency lookup and a process job with CPU/memory/child-process limits and kill-on-close. These mechanisms are resource controls for our included module, not general filesystem/network sandboxes for foreign code. Native Windows/Linux execution remains unaccepted. **Arbitrary third-party binaries are refused on every platform.** App signing and clean-machine acceptance remain distribution gates.

The worker supports offline stereo effects and CLAP note-driven instrument rendering, not live slots, external editors, sidechains or unrestricted plugin behavior. Worker crashes, rejection, output overflow and cancellation become failures, not a partially accepted audio result. A tested modified copy is rejected before loading. External DAW validation of our exported bundle remains pending.

## Recording

The common app opens input only after Record. It converts a selected pair or all pairs from 1–32 channels using fixed callback scratch storage, then publishes finite clamped samples to preallocated SPSC queues. The producer performs no allocation, locks, file access or monitoring. One disk worker writes private float32 WAV originals, staggers header checkpoints across pairs and finalizes/fsyncs on stop. Queue overflow, device interruption or the embedded-project budget stop capture; partial originals remain. Aggregate capture is at most four million frames and 120 seconds. The UI checks free space before starting and once per second, retaining 3 GiB; this is not an atomic filesystem reservation.

The earlier native AudioQueue adapter has a separate 30-second mono/48 kHz path. A write guard prevents overlapping mutation, stop flags prevent further requeueing and synchronous disposal precedes reading. Failed disposal retains callback memory to avoid use-after-free. It is not the common app's multitrack disk writer.

A completed/interrupted take is retained, inserted transactionally or offered for explicit WAV export when project capacity is exhausted. Closing/changing a session is refused while an uninserted take remains. Automated tests do not open the microphone or grant OS recording permission.

## Dependency and financial safety

Four repository-owned root Cargo packages have no registry dependencies or external build scripts. `pnpm run audit` enforces that boundary. Builds are offline, use at most two jobs (one for cross builds) and preserve a 3 GiB active free-space reserve; fresh desktop/cross builds require 5 GiB and explicit reuse of existing outputs requires 3.5 GiB. The app has no paid network API, so backend Denial of Wallet is absent. Local exhaustion and native driver/ABI failures remain relevant.

Official sources checked 2026-10-27/28:

- [Rust 1.99.1](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/) is the pinned official toolchain. Its FFI regression fixes are relevant to native boundaries.
- [Cargo CVE-2026-5223](https://blog.rust-lang.org/2026/05/25/cve-2026-5223/) affects Cargo before Rust 1.96.0. Our 1.99.1 toolchain is newer and this project uses no alternate registry.
- [rustup 1.29.2](https://blog.rust-lang.org/2026/10/20/Rustup-1.29.2/) addresses the certificate issue identified in CVE-2026-111943 / RUSTSEC-2026-0382. The installed manager reports 1.29.2; it is development tooling, not an application runtime.
- The [arrayref supply-chain incident](https://blog.rust-lang.org/2026/08/20/supply-chain-attack-on-arrayref/) and [targeted maintainer attacks](https://blog.rust-lang.org/2026/09/17/targeted-attacks/) were reviewed. Those registry packages are absent here.

The direct public CLAP security-advisory page and an earlier RustSec feed request failed to fetch. The official RustSec scan was subsequently completed and refreshed on 2026-10-31: zero known vulnerabilities and three unsuppressed maintenance warnings; see DEPENDENCIES.md. No exhaustive scan, absence of zero-days or claim of 100% security follows. Review new dependency/SDK/license changes and current advisories before distribution.

## Remaining acceptance

No formal verification, sanitizer/Miri run, sustained fuzz campaign, hardware latency bound, acoustic safety guarantee, physical recording acceptance or unrestricted third-party plugin safety is claimed. Offline digital ceilings do not establish safe sound pressure at a user's headphones. Full reference coverage and distribution gates remain in PARITY.md.

## Common desktop dependencies and callbacks

The shared UI/core adapters forbid unsafe Rust. Vetted basic libraries supply windowing, WGPU, file dialogs, accessibility, audio and free-space queries; their native implementations remain trust boundaries. The common app enables no URL-opening helper, inspection listener, telemetry or persistence plugin. The complete locked graph and current RustSec findings are recorded in DEPENDENCIES.md; three maintenance advisories remain unsuppressed.

CPAL callbacks accept only bounded, aligned channel buffers. Output consumes a fixed SPSC queue fed by a block-rendering worker, maps nonfinite samples to silence and limits each output sample to 0.4 in magnitude. Unsupported formats/configurations fail without starting a stream. Capture allocates queues before starting and publishes whole frames to the disk writer. Finishing drops the input stream, drains published frames and inserts the takes transactionally. There is no input monitoring, system-volume change or automatic stream at startup.

Rapid pointer clicks and per-event keyboard modifiers are handled independently of the next redraw. A focused toolbar button and a global shortcut cannot activate the same transport command twice. Export settings bind to the project revision; background scan results from a changed project are discarded. These tests do not establish physical device reliability, universal memory safety or absence of unknown vulnerabilities.

## V1 release policy

The 2026-11-03 RustSec scan has zero known vulnerabilities and three maintenance warnings. The exact warnings remain in the official JSON result. A separate, time-limited policy accepts only those unmaintained package artifacts until 2026-11-17; all vulnerabilities and unrelated/changed/expired warnings fail. The strict scanner result remains available through `pnpm desktop:audit --strict`. Policy regression tests cover vulnerability, expiry, changed checksum/version/source, unknown warnings and malformed reports. See DEPENDENCIES.md and security-maintenance.toml.

Release workflows run on version tags and pull requests. Only the tag-release job has repository-content write permission, after native checks and the current audit pass. Pinned actions stage three archives; the publisher validates their source commit, version, hashes and native first-party-plugin reports, uploads to a draft release, checks GitHub's uploaded sizes/digests, then publishes. It never replaces a different existing release asset. Credentials stay in the scoped workflow environment and are never written into source or package metadata.
