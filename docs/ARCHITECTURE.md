# Architecture

## Core

The Rust library forbids unsafe code. `model` validates musical state. `routing` compiles post-fader routes into a topological order, rejects cycles/duplicate destinations and propagates solo audibility through downstream buses. The engine preallocates 64 voices and effect storage, compiles clips to sample timestamps and processes fixed channel scratch arrays without allocation or locks.

`dsp` contains original oscillators, resamplers, effect processors and the output guard. `notes` implements selected-note clipboard and transformations. `edit` handles immutable sample copies/splices, with sinc sample-rate conversion. `stretch` is an experimental granular processor; transparent time/pitch quality is not claimed.

`codec` reads native versions 1–5 and writes version 5. Notes carry compact checked channel/source IDs; playlist rows and instrument channels are independent. Version 4 added these IDs and playlist row mutes; version 5 adds direct audio clips and clip phase/group/mute metadata. Old sessions bind legacy notes to their previous clip channels; reused patterns are cloned where needed, preserving arranged audio. An unarranged legacy pattern has no recorded channel, so migration uses its old pattern index when available. `flp` retains an opaque original/event index and maps supported notes/tempo and selected uncropped pattern playlists without executing plugin or sample references. Source note IDs retain opaque note fields after reorder. Unsupported edited exports fail instead of silently discarding changes. `midi` and `wav` implement their binary formats directly.

`flac` implements constant/verbatim/fixed prediction encoding and Rice residuals. Its bounded decoder also handles LPC, Rice2, escape residuals, wasted bits and stereo decorrelation. Frame CRC8/16 and nonzero STREAMINFO PCM MD5 values are checked; zero means the checksum was not supplied. The owned streaming digest uses a fixed 64-byte block. Encoding writes the digest of interleaved, signed, byte-aligned PCM before publishing the file. This is format integrity, not cryptographic authentication. Pictures/URLs and other metadata are skipped. The implementation follows [RFC 9639 §8.2](https://www.rfc-editor.org/rfc/rfc9639.html#section-8.2); its third decoding example and the published [RFC 1321](https://www.rfc-editor.org/rfc/rfc1321) digest vectors are tested. Apple PCM24 cross-checks are recorded in the evidence ledger.

`aiff` implements bounded AIFF/AIFF-C integer/float input and AIFF PCM16/24 output, including extended sample rates, SSND offsets and supported byte orders. `export` prepares immutable mix/channel/pattern snapshots and WAV/FLAC/AIFF settings. Pattern exports pad to complete bars. `stems` renders active channels sequentially with identical start/duration and downstream routing/sends. The UI writes each batch to a new folder with a disk-budget check; cancellation preserves completed files. Whole-song offline export remains bounded to 120 seconds / 96 MiB. Lossy codecs and long disk-streamed export remain absent.

`storage` reads bounded regular files with no-follow controls and publishes new files without replacing existing data. `recovery` writes quota-bounded private copies without pruning. `history` validates named transactions and supports seeking through retained previous/future states, sharing immutable sample/source buffers and counting retained payloads once for its estimated budget. `preferences` separately persists theme/audio/ghost-note choices and at most sixteen recent absolute paths in a bounded CRC32-checked private file. Corrupt configuration is preserved and blocks automatic replacement. Preferences never modify musical state.

## Common desktop

`apps/desktop` owns the shared eframe/egui/WGPU window. Canvas/layout, gestures/workspace, action dispatch, typed export settings, forms and file dialogs are separate modules. It reuses the own model/DSP and the independently drawn reference geometry. The safe UI forbids unsafe code. Named toolbar controls and standard forms use AccessKit; the canvas is not fully accessible yet.

CPAL output consumes a bounded queue with rate conversion and finite-level cleaning. Its callback allocates nothing and takes no locks. Capture maps a selected pair or all pairs from 1–32 input channels through fixed scratch storage into per-pair SPSC queues. `recording` owns one disk worker, private float32 WAV originals and staggered header checkpoints. Input opens only on Record. Stop drains published frames before finalization; interrupted originals and uninserted samples remain available. System backends, permission, physical alignment and power-loss behavior remain outside the synthetic tests' evidence.

Rendering, export, recovery and CLAP scanning use bounded workers. Scanning does not block the UI; stale results after session edits are discarded. Block playback streams through a fixed queue and still stops on edits; live graph exchange is absent. The nested lockfile keeps the native comparison build independent while the common desktop is validated.

`crates/plugin-host` shares the worker protocol and exact first-party package/hash gate across macOS, Windows and Linux. Native loaders and per-OS resource limits are separate modules; Windows/Linux execution remains pending. External plugins are rejected on every system. `crates/apple-dispatch` is a small owned compatibility patch forwarding the older Foundation wrapper's synchronous main-queue call to maintained dispatch2; it is not an audio callback dependency.

## Native application

`macos.rs` is the main-thread AppKit/worker adapter. `macos/workspace.rs` manages gestures; `workspace/reference.rs` draws the independently recreated screenshot layout. A 1920px design space scales with the native window; transparent native controls retain keyboard actions, tooltips and accessible names while the canvas draws their appearance. `theme.rs` provides primitives and process-scoped CoreText registration for the upstream Cuprum/Tabler fonts. `actions`, `menus` and `files` separate command dispatch, forms and session I/O. A gesture previews its baseline snapshot and commits one history operation on release.

Playback uses an immutable prepared loop. Synthesis/export/CLAP/recovery run in bounded background work; the AudioQueue output callback only copies frames. Editing stops prepared playback and the next Play rerenders. This design is not a streaming/live DAW engine.

`capture_macos` opens input only on Record and fills a preallocated 30-second take. No monitor is connected to output. The UI polls completion and retains received audio after interruption. The microphone, physical timing and permission flow still require human/device acceptance.

## CLAP boundary

`crates/clap-abi` contains repository-owned public C ABI declarations. `plugins/garrido-native` builds a separate CLAP bundle with fourteen descriptors backed by our engine/DSP. Parameters, state, stereo/note ports, processing and reset are implemented. No vendor DSP or plugin framework is incorporated.

`plugin_host` runs only in a worker invocation of the owning executable. It spawns our own child with a minimal environment, applies per-OS resource controls before loading, and exchanges bounded bytes across pipes. Effects transform copied samples. Instruments receive timestamped CLAP note events and produce a sampler render; the source channel is preserved and muted. The GUI never receives plugin pointers. Only the manifest-matching packaged first-party module is accepted on all systems; general third-party isolation remains unfinished. See SECURITY.md.

## Budgets

| Resource | Current bound |
|---|---:|
| Channels / patterns / notes / clips | 32 / 256 / 32,768 / 4,096 |
| Voices | 64 |
| Routing edges / per-channel effect slots | 256 / 8 |
| Embedded sample frames | 4,000,000 stereo frames |
| Effect delay storage | 4,000,000 stereo frames |
| Render duration / buffer | 120 seconds / 96 MiB |
| Voice / estimated graph-effect work | 80 million voice frames / 160 million operations |
| Undo | 32 snapshots; estimated 32 MiB of retained history beyond current shared payloads |
| Native/WAV/FLAC/AIFF input / FLP / MIDI | 64 MiB / 32 MiB / 8 MiB |
| FLAC decoded frames | 4,000,000 |
| Output | 128 MiB |
| Common input capture | 1–16 stereo pairs, aggregate four million frames, at most 120 seconds |
| Preferences / recent paths | 64 KiB / 16 |
| Stem batch maximum | 512 MiB estimated output |
| Recovery | Eight copies and 128 MiB; no automatic deletion |
| Build/recovery disk reserve | 3 GiB |

These are implementation budgets, not proofs of total process memory, device deadlines or crash/power-loss resilience. Platform backends, physical acceptance, live graph exchange, CoreMIDI and the rest of PARITY.md remain future work.


## Block playback and direct audio clips

The common application prepares an Engine off the UI thread, then generates blocks on a dedicated worker. An owned 8192-frame SPSC queue transfers stereo floats through atomics; its producer and consumer are unique Rust endpoints. The callback allocates no queue storage, takes no locks and writes silence to missing/extra channels. Backpressure pauses the worker, underruns are counted, and stopping joins only the owned worker. No disk or plugin I/O happens in this callback. Loop boundaries reconstruct the engine on the worker; this is not live graph exchange, a physical latency result or a guarantee against driver scheduling failures.

`Engine::streaming` compiles long songs with bounded event/effect budgets without reserving whole-song audio. Whole-song `render()` rejects streaming instances before allocation. The separate offline constructor retains its duration/buffer/work bounds; making playback longer does not bypass export limits.

Format 5 adds direct audio clips plus pattern offset, mute and group metadata; readers retain versions 1–4. Audio clips reference their channel's immutable shared sample, a source offset and an anchor. Splitting retains the anchor, so resampling phase is identical across the cut. Move preserves the starting source position across tempo changes. Compatible joins never concatenate or resample source files. Pattern cuts through held notes fail explicitly while between-note cuts retain loop phase. Pattern-only export excludes audio clips.
