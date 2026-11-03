# Desktop dependencies

The audio core, native AppKit application and own CLAP plugin contain no registry dependencies. The shared desktop preview has its own lockfile in `apps/desktop`, including all platform-specific transitive dependencies. The root workspace now contains four owned packages; this does not mean the shared desktop has zero dependencies.

Direct dependencies are pinned: eframe/egui 0.37.0 for the shared window, widgets, accessibility and WGPU rendering; CPAL 0.18.2 for system audio; rfd 0.17.2 for native file dialogs; fs4 1.1.0 for the available-space guard. The application does not enable eframe's URL-opening, inspection server or persistence features. Existing system libraries, display servers and file-dialog portals retain their own security boundaries.

## Security evidence, 2026-10-28

RustSec cargo-audit 0.22.2 was downloaded from the official RustSec GitHub release for aarch64 Apple Darwin. The archive SHA256 was verified against the release asset digest before extraction/execution: `ec7ca4263769593df4d909be85b94a6b79efa2897be5d2bb8ebd516e823175af`. The tool is local development tooling in ignored `.toolchain/`, not an application dependency.

The official advisory database snapshot contained 1,360 records. The shared desktop lockfile contains 401 registry packages, including dependencies for platforms that are not being shipped. The official scan found zero known vulnerabilities and three maintenance warnings:

- RUSTSEC-2026-0370: zvariant 5.15.1.
- RUSTSEC-2026-0371: zbus_names 4.3.5.
- RUSTSEC-2026-0372: zvariant_derive 5.15.1.

The standalone crates were merged into zbus 6; the advisories describe discontinued maintenance, not a disclosed security exploit. They are reached by the Linux accessibility dependencies. No warning is suppressed and accessibility has not been disabled to hide them. `pnpm desktop:audit --strict` retains the official scanner's warnings-denied result. The v1 release policy additionally permits only the exact, dated maintenance reviews in docs/security-maintenance.toml; vulnerability findings and other warnings remain blocking. A zero-result advisory scan does not establish absence of unknown vulnerabilities.

`outputs/desktop-official-audit.json` retains the official result; `outputs/desktop-dependency-audit.json` binds its summary to the lockfile hash. The local extracted advisory database avoids another network fetch; refresh it before release. The scanner additionally checks the local registry's yanked-package records. This snapshot is evidence for its retrieval date, not ongoing monitoring.

## Licenses

Direct libraries are permissively licensed: eframe MIT OR Apache-2.0, CPAL Apache-2.0, rfd MIT, fs4 MIT OR Apache-2.0. All 401 declared transitive expressions were inventoried in ignored `outputs/desktop-license-inventory.json`. Several multi-license packages offer a GPL or LGPL alternative; their permissive alternative must be selected and its notice included. No GPL/LGPL option is selected merely because it appears in an OR expression.

The collection is retained in `legal/desktop-dependencies/`, with package versions, registry checksums, license expressions, file hashes and upstream provenance. `pnpm run desktop:licenses` locates notices for all 401 registry packages. This inventories notices; it does not establish legal clearance for the complete application or its visual presentation.

Some egui 0.37.0 crate archives name a source revision that is not publicly retrievable. Their license texts were taken from the official 0.37.0 release tag, resolved to `841a328ba2dd8f6996beb7189ebb7fc00e668a2a`; the original crate revision remains recorded separately. No claim of source-code equivalence to that tag is made. Crate integrity is tied to Cargo.lock's registry checksum. Upstream Git blob hashes and local SHA256 values protect the retrieved license texts.

The old `dispatch` 0.2 registry crate did not supply a license text in its archive or pinned repository tree. It has been removed from this dependency graph. A small owned compatibility adapter in `crates/apple-dispatch` implements only the `Queue::main().exec_sync` interface used by objc2-foundation 0.2, forwarding to the already-used dispatch2 0.3.1. It contains no unsafe code and no old dispatch source. Its return-value behavior is preserved. This is a targeted compatibility patch, not a general replacement implementation of the upstream crate.

Dependency expressions offering GPL/LGPL alternatives retain those declarations, but only their permissive alternative is selected for this application; the r-efi AUTHORS file contains the corresponding copyright and MIT grant. The application's restrictions do not replace any third-party permission. Build and distribution tools are separate from the application graph.


On local 2026-10-29 the official RustSec snapshot was refreshed to `40f5be6fb12ed93fc591bf4ab23b7d793b3f2239`, 1,361 records. Archive SHA256: `152927bb0cb1b749eb79f740b45e0d28825e38862fee4b8835af92721c6db243`. `pnpm run desktop:audit --db outputs/rustsec-db-20261029` records the database path and check time and still fails for the same three maintenance warnings, with zero known vulnerabilities.

A local Windows cross-link experiment uses the official Zig 0.17.0 Apple Silicon SDK, archive SHA256 `b607e9b9234790a008116ae5bdb71c6243b84b9fb42a53a9e70fde41c06c536a`, verified against Zig's official release index before extraction/execution. Its extracted size is 388,028,117 bytes. The official public GitHub advisory list returned empty; that is a source check, not a claim of complete vulnerability coverage. This ignored development SDK is not an application dependency. `pnpm run desktop:build:windows` uses one build job and aborts its owned process group if physical free space falls below 3 GiB or the bounded build time expires. GNU CRT linkage was not compatible with this SDK. LLVM/UCRT core and plugin linkage now works through explicit CRT selection and its official compiler runtime; common GUI linkage and Windows execution remain separate checks. No runtime library symbols were stubbed. No Windows executable is accepted solely from type checking.


The cross linker now builds the unmodified `lib/compiler_rt.zig` from the verified SDK as a normal static compiler-runtime library and links that library explicitly. It supplies the full compiler implementations of combined trig and long-double math; no missing symbol is replaced by an empty or hand-approximated function. SDK MIT, musl copyright, MinGW and LLVM libunwind notices are retained with hashes under `legal/windows-runtime/`. These are compiler/runtime components, separate from the independently implemented Rust audio algorithms; the notices inventory is not legal clearance for the entire product.

## Advisory refresh, 2026-10-31

Official RustSec revision [`2295466c4e9df26647fd67d8fa1c705ac2fc7f81`](https://github.com/rustsec/advisory-db/commit/2295466c4e9df26647fd67d8fa1c705ac2fc7f81), committed 2026-10-30 11:12:39 UTC, contains 1,375 records. Archive SHA256: `bb962bdfe1ed50e91a82c819acaa40865b6c35e380856f3cd751d71f95b72dd3`. The bounded extraction reads advisory markdown only; it executes no downloaded code. The audit at 2026-10-31 04:38:36 UTC again reports zero known vulnerabilities and the same three unsuppressed maintenance warnings for the unchanged 401-package lock (`83941c3a4513edf2c8b630c4b265b648f9ce3a085c1faae8a0225cbcd785460b`). `pnpm desktop:audit --db outputs/rustsec-db-20261031` remains failed.

The official AccessKit Unix 0.24.1 dependency declarations still use zbus 5 / AT-SPI 0.29; AT-SPI 0.30 still uses the standalone zbus 5-family crates. zbus 6 is not a compatible lockfile-only update. No unreviewed dependency fork, warning suppression or removal of Linux accessibility was used. Migration and review remain pending. The new AIFF, preferences, recording, SHA256 integrity and platform-host code add no registry dependency. SHA256 here checks packaged bytes, not publisher identity or a FIPS certification.


Linux cross-linking uses actual libraries, headers and .pc files from 11 official Ubuntu noble/noble-security packages. Every Debian archive was checked against its official package-index SHA256; only needed library/header/metadata files were extracted, and Debian maintainer scripts were not run. Provenance is in ignored `outputs/linux-sysroot-provenance.json`. The host metadata reader is official pkgconf 3.0.10, source archive SHA256 `9371b2f6372da574d4c6dc96fe9b1463037bcf6afcb6671c1555afde7b17e2de`, compiled locally without installing it into macOS. The official public advisory list was empty. The existing zstd 1.5.7 development library was used only for bounded package extraction; its official latest release/advisory list was checked. None of these development tools or Ubuntu shared libraries is bundled in the Linux app. The Linux app uses the user's installed system libraries. A syscall/desktop/ALSA acceptance test is still required.

## V1 review, 2026-11-03

The official snapshot is revision `263a4d951a0867d4bddc29dddd6d59427c78f9a7`, committed 2026-11-02 11:45:44 UTC, with 1,382 records. Archive SHA256: `eb03b97ab7d77d07bcd1d8287d388de6ef1356ddf866565ea175593c1a2ef228`. The scan finds zero known vulnerabilities and the same three unmaintained warnings. The registry graph and its checksums are unchanged; only owned packages move to 1.0.0.

These three exact maintenance findings have a review expiring 2026-11-17 in security-maintenance.toml. It binds advisory ID, package, version, registry and checksum. The raw official report still includes every warning and records its strict failure. Release-policy approval is a separate field. Expiry, a changed artifact, a new warning or any vulnerability blocks the check. `pnpm audit:policy:test` exercises those refusal cases. No vulnerability is ignored and no Linux accessibility feature is removed. Contribution work should migrate the complete AT-SPI/AccessKit chain to zbus 6 and remove the temporary review.
