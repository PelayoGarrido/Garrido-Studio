# Garrido Studio 1.2.0 · Spectral Journey

By **Pelayo Garrido Martínez**.

Spectral Journey is a native sound-design workflow that creates a new evolving texture from two short tonal fingerprints. Frequencies travel between the sources in log pitch space. Holding a position creates a fixed hybrid; reversing Start/End creates a return journey.

Open **Tools → Spectral Journey → Load example pair → Render**. The graph shows the actual frequency routes. Audition the journey against a conventional crossfade with matched overall RMS, then add a new clip with Undo or export a WAV. You can also load local WAV/FLAC/AIFF files or use a sample channel. No microphone is opened by this workflow.

The FFT, peak analysis, monotone one-dimensional transport and additive synthesis are implemented in Rust in this repository. There are no new registry dependencies, model downloads or cloud processing. Analysis/rendering use a cancellable worker; the audio callback consumes completed audio. Sources remain immutable and insertion checks the session revision.

This is tonal resynthesis, not faithful reconstruction of speech, rhythm, original phase or full recordings. Sources are limited to 12 seconds and 24 MiB per file, with 1–32 modes. Output duration is 0.25–12 seconds. The existing project-memory and channel limits still apply. Spectral morphing has established research and product precedents; [the implementation notes](https://github.com/PelayoGarrido/Garrido-Studio/blob/main/docs/SPECTRAL-JOURNEY.md) explain the method, prior work and limits.

The website includes actual matched PCM demo renders. `pnpm spectral:demo` reproduces the comparison, source-route data, session and timing report without opening an audio device.

## Distribution

Packages cover macOS 13+ Apple Silicon, Windows x64 and Linux x86_64/glibc 2.39 with ALSA and a graphical desktop. Extract the complete folder. SHA256SUMS.txt identifies the package bytes. macOS is ad-hoc signed; Developer ID/notarization and Windows publisher signing remain pending. Physical audio-device and long-session acceptance remain separate from automated native checks.

The approved logo, moodboard and initial dark theme are retained. FLP interoperability and external plugin boundaries remain documented in CONTRIBUTING.md. The included module still provides eight effects and six instruments.

## License

PolyForm Noncommercial 1.0.0 with the commercial music permission. Producing and selling your own music/audio is allowed. Monetizing the program or its forks requires separate permission. Third-party components retain their own licenses.
