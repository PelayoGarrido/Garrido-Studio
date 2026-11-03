# Contributing to Garrido Studio

Garrido Studio v1 is a working foundation for making, arranging and exporting music with an original Rust engine. Contributions should improve a complete user flow, preserve musical data and include evidence for the behavior changed.

## Useful work

| Area | Next contribution |
|---|---|
| FLP | Additional version/playlist fixtures, cropped and audio clips, mixer/plugin/automation interpretation and full writer coverage. Existing note/tempo support is partial; retain opaque events and unsupported-edit refusal. Never implement trial or license bypasses. |
| Plugins | General third-party CLAP isolation, live slots, state/presets, parameter automation and editors; VST3 host/export work. V1 currently accepts only its exact packaged first-party module. |
| Recording | Device/permission/unplug and timing acceptance, count-in, latency compensation and file-backed long sessions. Current embedded samples total at most four million stereo frames; capture stops within 120 seconds, often sooner depending on rate and active pairs. |
| Playback | Live graph exchange, note audition and MIDI devices/recording/controllers/sync. Editing currently stops playback. |
| Export | Long disk-streamed rendering, regions/metadata and lossy codecs. Whole-song offline rendering currently has a 120-second / 96 MiB bound. |
| Editors | Automation clips, additional arrangements, complete piano/event tools, persistent layouts and canvas accessibility. |
| Distribution | Clean-machine testing, Windows publisher signing, macOS Developer ID/notarization and Linux desktop/install packaging. |
| Accessibility dependencies | Move the Linux AT-SPI/AccessKit chain to zbus 6 as upstream support permits. Three exact maintenance advisories have a time-limited review in docs/security-maintenance.toml; vulnerability advisories are never exempted. |

## Platform reports

Use the included example and [platform checks](docs/PLATFORM-ACCEPTANCE.md). Report OS/version, architecture, display scale, audio device/rate, app version and the smallest reproducible steps. Keep originals untouched and attach a newly generated test project only if you have the right to share it. `Check-Plugins` generates an offline report; it does not validate a third-party plugin, a microphone or listening quality.

Local macOS testing and cross-linked Windows/Linux packages are separate evidence. Native CI checks cover model/editor behavior and the included plugin; real sound devices, permissions, display scaling and clean-machine behavior still need human acceptance. A package or test result is not a guarantee for every driver or system. The current [evidence ledger](docs/BUILD-EVIDENCE.md) records exactly what has run.

## Changes

Use the pinned toolchain and pnpm commands from README.md. Keep DSP, codecs and project semantics in the core; keep OS integration in adapters. Avoid allocations, locks, file access and system calls in audio callbacks. Validate data and budgets before committing a musical edit. Preserve existing files and interrupted takes. Do not load a library, follow a URL or read arbitrary sample paths because a project requests it.

Add focused regression coverage for parser, audio or data-preservation changes. Run the relevant checks and describe how the result was verified. Do not claim listening, hardware or platform acceptance from a compile-only result. Keep comments short and useful. Submit work you own or have permission to contribute; include provenance and notices for dependency or asset changes.

Contributions remain subject to the repository license and existing third-party terms. Do not assume an outside contribution can be commercially relicensed without its author's permission. Agree any separate commercial grant with the maintainer explicitly.

## Security reports

Use the private vulnerability reporting option on GitHub when available. Do not include credentials or private projects in a public issue. Include the affected version, a minimal reproduction and the boundary crossed. Do not execute an unknown plugin to test a report on a personal machine.
