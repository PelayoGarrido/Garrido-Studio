# Garrido Studio 1.0.0

First public release by **Pelayo Garrido Martínez**.

An independently implemented Rust workstation with a shared desktop UI, pattern sequencing, multichannel piano roll, audio arrangement, mixer/routing, original instruments/effects, named history, native sessions, disk recording and WAV/FLAC/AIFF export with channel stems.

The approved logo and moodboard define the coral/rose/violet identity. Dark mode is the initial default; light is an explicit preference. The app runs locally without an account, telemetry, model downloads or a cloud backend.

## Packages

- macOS 13+, Apple Silicon arm64.
- Windows x64.
- Linux x86_64 with glibc 2.39, ALSA and a graphical desktop.

Extract the complete folder and keep its resources/plugins together. The example session, notices, platform guide and offline included-plugin checker are supplied. Release assets are identified by SHA256SUMS.txt. macOS signing is ad hoc; notarization and Windows publisher signing remain contribution work.

## Validation

The release runs formatting, strict Rust Clippy, core/plugin tests, common-editor tests, resource/data-preservation tests and an offline diagnostic for all fourteen included plugins. macOS codec interoperability is cross-checked with Apple's conversion tools using a generated PCM fixture. Automated checks never open a microphone or play a musical signal.

Native CI logs and the checked-in evidence ledger are the source of actual test results. See CONTRIBUTING.md for FLP coverage, first-party plugin scope, duration/storage budgets, supported workflows and further development. No general third-party-plugin, driver, listening or power-loss guarantee is made.

## License

PolyForm Noncommercial 1.0.0 plus the commercial music permission: commercial music/audio production is allowed; monetizing the software requires separate permission from the licensor.
