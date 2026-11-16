# Third-party components

The application is licensed under GPL-3.0-or-later. The following native runtime
is supplied for local development and packaged with the Windows application.
Rust and frontend dependencies additionally retain their respective licenses,
recorded in Cargo.lock/package-lock.json and the upstream packages.

## libmpv / mpv

- Upstream: https://github.com/mpv-player/mpv
- Integration: dynamically loaded C client API, not a modified mpv source tree.
- License: mpv is GPL-2.0-or-later by default; some specially configured builds
  can be LGPL. This project treats the supplied third-party build as GPL.
- Native package provider: https://github.com/shinchiro/mpv-winbuild-cmake
- Provider referenced by mpv: https://mpv.io/installation/
- Pinned release: https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20261111
- mpv revision: https://github.com/mpv-player/mpv/tree/bcde6b93cc
- Build scripts: https://github.com/shinchiro/mpv-winbuild-cmake/tree/bda10a2
- Build job: https://github.com/shinchiro/mpv-winbuild-cmake/actions/runs/42927840447
- Artifact: `mpv-dev-x86_64-20261111-git-bcde6b93cc.7z`, baseline x86-64.
- Archive SHA256: `273c079b11ea5575822f972756f4c9a71149a5c10130353abeb8586738938452`.
- `libmpv-2.dll` SHA256: `88504ee5560942a955405e99dfb5971368e4cf851321f36565bf2a667271db1b`.

The pinned build incorporates FFmpeg and other native dependencies. Source and
build configuration references are listed above; FFmpeg revision:
https://github.com/FFmpeg/FFmpeg/tree/cd4e96eb3 . Each incorporated dependency
retains its license. Bundled mpv license texts are under
`src-tauri/resources/mpv/`. The build manifest records binary provenance.

Before publishing a binary distribution, collect the complete dependency license
inventory and corresponding source/build material for this exact native build.
An upstream source link alone is not presented here as a complete source offer.

## FFmpeg analysis and editing runtime

- Upstream: https://ffmpeg.org/
- Pinned Windows x64 package: Gyan FFmpeg 9.0.2 essentials.
- Archive: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip
- Archive SHA256: `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba`.
- Executable SHA256: `3256173f3f8bffd7df12227c68adf68025edb1832273a9530688a7bb1ed8edec`.
- Source revision: https://github.com/FFmpeg/FFmpeg/tree/946fcce07b
- License: GPL version 3, with vendor license, build configuration and provenance
  bundled under `src-tauri/resources/ffmpeg/`.
- Setup: `scripts/setup-ffmpeg.ps1` verifies the archive and executable hashes.

This standalone executable supplies loudness analysis, waveforms, preview and
edited-copy export. Playback continues to use the separate libmpv runtime.

## Rust components

- libloading: ISC, https://github.com/nagisa/rust_libloading
- Lofty: MIT OR Apache-2.0, https://github.com/Serial-ATA/lofty-rs
- rusqlite: MIT, https://github.com/rusqlite/rusqlite
- SQLite (bundled with rusqlite): public domain, https://sqlite.org/copyright.html
- WalkDir: MIT OR Unlicense, https://github.com/BurntSushi/walkdir
- SHA-2: MIT OR Apache-2.0, https://github.com/RustCrypto/hashes
- UUID: MIT OR Apache-2.0, https://github.com/uuid-rs/uuid

This file documents the native runtime and principal infrastructure components;
it is not a substitute for the full dependency license inventory of a published
release.
