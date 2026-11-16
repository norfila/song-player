# Changelog

## [0.5.0] - 2026-11-16

First public release.

### Added

- Music library: add folders, browse by album or folder, search and sort.
  Folders are rescanned in the background when files change.
- Playback of FLAC, ALAC, MP3, AAC/M4A, Opus, Vorbis, WAV/AIFF, WMA, WavPack
  and APE through libmpv.
- Several queues, each with its own position, shuffle and repeat mode, and a
  sleep timer.
- Playlists and favorites, with M3U/M3U8 import and M3U8 export.
- Now Playing view, Windows media controls, media keys and a tray menu.
- Loudness matching with ReplayGain analysis per track, album or folder, and a
  five-band equalizer with presets.
- Quick search with typo tolerance for tracks and actions, optionally through
  a global Windows shortcut.
- Listening statistics by month, year and all time, stored locally.
- Audio editor that exports a selection as a new FLAC or WAV file.
- Light, Dark and Gold themes, custom themes, German and English, custom
  keyboard shortcuts, and settings backup with import preview.

### Changed

- Names left over from earlier project names were replaced. App data now lives
  in `%APPDATA%\io.github.norfila.songplayer`, and theme and settings exports
  use new format names. Test builds before 0.5.0 are not upgraded in place:
  uninstall them first. Their library, settings and export files are not
  picked up automatically.
