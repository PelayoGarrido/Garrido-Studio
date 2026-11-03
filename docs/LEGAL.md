# Legal provenance and publication gate

Requested author and licensor: **Pelayo Garrido Martínez**. Product name: **Garrido Studio**. The name is provisional; no trademark clearance or registration has been established.

## Reference boundary

Reference: ItsAlphaNeon/Lemon-Studio, revision `a6ce2dd443d75abf424a50fb5dd68526f87bed9d`, retrieved on 2026-10-27. Its public README, capability table, status description, manifest, commit metadata and workspace screenshots were consulted to inventory the target behavior. The implementation is written independently in a separate repository. No reference source or artwork is vendored, translated, ported or incorporated.

This is an independent implementation process, not a claim that a formally separated multi-person clean-room process has occurred. A metadata response included an unrelated patch, which was not used. Do not characterize this project as a license conversion of Lemon Studio. Copying its GPL source later would require a fresh compatibility review and would invalidate the current independent-source assumption.

## License choice

GPL is unsuitable for the requested restriction: it permits commercial sale and distribution. The selected baseline is the official unmodified PolyForm Noncommercial License 1.0.0, together with a separate grant for commercial music/audio production. The latter applies to audio outputs and production services, not to selling the software or monetizing a fork, hosted software service or commercial software integration.

The owner explicitly confirmed on 2026-10-27 that commercial music production is permitted while selling or monetizing the program and its forks requires the owner's permission. Keep both terms and NOTICE with every distribution. Commercial exploitation of this software requires separate permission from the licensor. The licensor can offer the licensor's own work under other terms in the future. Existing grants and third-party licenses must still be respected.

This is source-available software, not OSI open source. Noncommercial licensing cannot be represented as GPL, MIT or another unrestricted open-source license. Ownership and terms for future outside contributions must be agreed before accepting them; do not assume every contributor's work can later be relicensed commercially.

## Content and interoperability

No FL Studio factory content, recordings, patches, proprietary SDK, decompiled code or commercial reference projects are included. The default demo uses generated waveforms and noise. User-imported audio/MIDI remains the user's responsibility. MIDI/WAV interoperability is implemented from format structure; sample comparisons against other products do not establish rights to their assets.

FLP pattern-note/tempo interoperability is implemented from public format descriptions, with opaque preservation and explicit unsupported-edit refusal. No trial/license bypass or factory content is implemented. CLAP public API layouts were independently declared in Rust; their upstream MIT notice is retained in `legal/CLAP-LICENSE`. The exported plugin contains our own DSP. FLAC is independently implemented from [RFC 9639](https://www.rfc-editor.org/rfc/rfc9639.html), without extracting its code components or bundling a reference codec.

FL Studio, CLAP, FLAC and VST identify interoperability formats; no affiliation, endorsement or trademark permission is implied. The [official VST3 licensing documentation](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Index.html) was checked: VST3 3.8 uses MIT terms. This does not implement a host/exporter or establish VST2 rights. No VST/ASIO code is incorporated.

The owner requested close visual matching of the supplied screenshots on 2026-10-28. Geometry, colors and controls are independently drawn in Rust. Cuprum and Tabler fonts come directly from their upstream Google Fonts / official Tabler registry distributions, under OFL 1.1 and MIT respectively, with notices and integrity hashes retained. They were not extracted from Lemon Studio. These third-party assets retain their own permissions, including commercial rights under their respective licenses; the application's license cannot remove those permissions.

The reference's Cuprum 1.001 font was downloaded into ignored research output for metadata comparison only. It is not an application asset. The packaged font is the unmodified Google Fonts 1.002 distribution with its own OFL notice; the 1.001 file's exact distribution provenance was not independently established. Accordingly, exact glyph identity is not claimed.

Close visual matching is not a legal opinion on copyright, design rights or trade dress. A small visual change alone would not authorize copying protected artwork or GPL code. This independent-source record does not establish clearance to distribute an identical visual presentation. Review those rights before publication or commercial licensing, in addition to the functional release gate.

## Before publication

1. Match release claims to executable evidence; retain the current boundaries and contribution roadmap in PARITY.md and CONTRIBUTING.md.
2. Confirm final name, license grant, copyright notices, contributions and distribution notices.
3. Complete repository/privacy review, actual supported-platform testing and any needed signing/notarization.
4. Publish the documented version scope, with correct notices, checksums and platform evidence. Full reference parity remains a development roadmap.

Primary sources: [GNU FAQ on charging](https://www.gnu.org/licenses/gpl-faq.html#DoesTheGPLAllowMoney), [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0), [reference license](https://github.com/ItsAlphaNeon/Lemon-Studio/blob/a6ce2dd443d75abf424a50fb5dd68526f87bed9d/LICENSE).
