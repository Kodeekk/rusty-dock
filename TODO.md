# rusty-dock improvements tracker

## Completed

- [x] Remove ImageMagick dependency - use image crate + resvg for icons
- [x] Add folder popup animation - scale-in and fade-in effects
- [x] Add tooltip delay (500ms) before showing
- [x] Fix PID tracking magic number (10_000) with proper struct
- [x] Add config validation with helpful error messages
- [x] Add import/export config functionality (CLI: --export, --import)
- [x] Add CLI help message (--help flag)
- [x] Add clippy configuration (clippy.toml)
- [x] Add full config window support for all config options

## High Priority

- [ ] Right-click context menu - quit, reload config, edit options
- [ ] Add clippy to CI pipeline (GitHub Actions)
- [ ] Improve error handling - replace unwrap() with proper Result propagation

## Medium Priority

- [ ] App search/launcher bar with fuzzy search
- [ ] Notification badges on app icons
- [ ] Drag-and-drop from external apps (desktop files, executables)
- [ ] Async process monitoring in background thread
- [ ] Config validation UI (show errors inline)

## Low Priority

- [ ] Keyboard navigation - arrow keys, Enter, Escape, Super key focus
- [ ] GPU acceleration with wgpu or glow
- [ ] Crash reporting (optional, with backtrace)
- [ ] Event-driven architecture using calloop channels
- [ ] Preloading icons on startup in background thread
- [ ] Config GUI live preview (border radius, opacity)
- [ ] Freedesktop notification integration for badges
- [ ] Hide dock on fullscreen app
- [ ] Auto-hide delay animation (slide out)
- [ ] Smart-hide trigger zone visualization in config window

## Backlog / Ideas

- [ ] Visual focus indicator for keyboard navigation
- [ ] Recent/frequent apps in search
- [ ] Config GUI non-blocking (always separate process)
- [ ] Icon bounce on launch animation (make configurable per-app)
- [ ] Favorite apps section in search
- [ ] Split large files into modules (main.rs, renderer.rs, config_window_iced.rs)
