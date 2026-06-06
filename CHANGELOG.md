# Changelog

## [0.2.0] — 2026-06-06

### Added
- Full shadcn-svelte component library integration (11 components)
- Tailwind CSS v4 with custom dark theme and shadcn compliance
- 10 audio format support (FLAC, MP3, M4A, AAC, ALAC, AIFF, OGG, Opus, WAV)
- Native Spotify playback via librespot (no Spotify desktop required)
- Provider credential management with .env, OS keyring, and Dev tab
- Album art display from embedded cover art
- EQ with 7 presets (10-band, parametric-ready)
- Crossfeed, sleep timer, AB repeat
- Crossfade between tracks (0–12 seconds)
- Playback speed control (0.5×–2.0×)
- Mono downmix and pre-amp gain
- MusicBrainz auto-tagging
- Smart playlists with rules engine
- Genre radio and discovery dashboard
- Library stats dashboard (formats, top artists, forgotten tracks)
- Watch folders (auto-scan new files)
- Duplicate finder
- Album grid view
- Fullscreen now-playing mode
- Accent color picker (6 presets)
- Layout density (compact/comfortable/spacious)
- TIDAL, YouTube Music, SoundCloud search/metadata
- Global media hotkeys (play/pause/skip/volume)
- MPRIS integration (Linux)
- System notifications on track change
- High contrast mode
- Screen reader support (ARIA labels)
- i18n foundation (EN/DE)
- CI workflow (GitHub Actions)
- E2E test setup
- Performance benchmarks
- Dev container config
- Keyboard shortcuts reference modal (`?`)

### Changed
- Rust edition 2021 → 2024
- Restructured Rust codebase: 19 flat files → 6 module directories
- All native HTML replaced with shadcn-svelte components
- Manual CSS (~3,000 lines) → Tailwind utilities
- "Audio" nav label → "Settings"
- Icon-only transport controls
- Responsive layout: desktop → tablet → mobile
- SQLite WAL mode with optimized indices

### Fixed
- Spotify playback now works without Spotify desktop open
- Dark theme dropdowns no longer transparent
- Error codes no longer shown in player card
- Tables no longer overflow horizontally
- 28 Svelte 5 `$state()` warnings fixed
- 7 Clippy warnings fixed

[0.1.0]: (initial version, pre-scrum)
