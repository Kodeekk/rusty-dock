# rusty-dock

A Wayland dock for Linux. Built with Rust, cairo, and smithay-client-toolkit.

## Requirements

- Wayland compositor (Hyprland, Sway, GNOME, KDE, etc.)
- ImageMagick (`convert` command)
- Cairo graphics library

## Building

```bash
cargo build --release
```

### Dependencies

**Arch:**
```bash
sudo pacman -S cairo imagemagick
```

**Ubuntu/Debian:**
```bash
sudo apt install libcairo2-dev imagemagick
```

**Fedora:**
```bash
sudo dnf install cairo-devel ImageMagick
```

## Running

```bash
# Start the dock
./target/release/rusty-dock

# Open config
./target/release/rusty-dock --config

# Show help
./target/release/rusty-dock --help
```

### Config CLI

```bash
# Export config to file
rusty-dock --export ~/my-config.json

# Import config from file
rusty-dock --import ~/my-config.json
```

## Auto-start

Add to your compositor's config:

**Hyprland** (`~/.config/hypr/hyprland.conf`):
```
exec-once = ~/.cargo/bin/rusty-dock
```

**Sway** (`~/.config/sway/config`):
```
exec ~/.cargo/bin/rusty-dock
```

## Configuration

Config file: `~/.config/rusty-dock/config.json`

You can edit it by hand or use `rusty-dock --config`.

### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `icon_size` | u32 | 48 | Icon size in pixels |
| `dock_height` | u32 | 64 | Dock panel height |
| `icon_padding` | u32 | 8 | Padding around icons |
| `border_radius` | f64 | 16.0 | Corner radius |
| `background_opacity` | f64 | 0.85 | Background transparency |
| `dock_margin` | i32 | 10 | Margin from screen edge |
| `position` | string | "bottom" | Dock position (top/bottom/left/right) |
| `auto_hide` | bool | true | Hide when not hovered |
| `auto_hide_delay_ms` | u64 | 500 | Delay before hiding |
| `smart_hide` | bool | true | Only reveal on stationary cursor |
| `smart_hide_trigger_delay_ms` | u64 | 1000 | Time cursor must stay at bottom |
| `smart_hide_y_threshold` | f64 | 6.0 | Max Y movement to be "stationary" |
| `enable_animations` | bool | true | Enable animations |
| `animation_duration_ms` | u64 | 200 | Animation duration |
| `launch_bounce_duration_ms` | u64 | 500 | Bounce animation duration |
| `icon_zoom_on_hover` | f64 | 1.3 | Zoom factor on hover |
| `enable_bounce_on_launch` | bool | true | Bounce when launching apps |
| `show_app_names` | bool | true | Show tooltips |
| `show_active_indicators` | bool | true | Show running app dots |
| `active_indicator_style` | string | "dot" | Style: dot/underline/border/glow |
| `grayscale_inactive_icons` | bool | false | Gray out inactive apps |
| `folder_popup_columns` | u32 | 3 | Folder popup grid columns |
| `folder_popup_icon_size` | u32 | 48 | Folder popup icon size |
| `show_folder_miniatures` | bool | true | Show mini icons in folders |
| `debug` | bool | false | Debug mode |
| `show_smart_hide_zone` | bool | false | Show trigger zone (debug) |

### Example config

```json
{
  "icon_size": 56,
  "dock_height": 72,
  "icon_padding": 10,
  "border_radius": 12.0,
  "background_opacity": 0.9,
  "auto_hide": true,
  "smart_hide": true,
  "pinned_apps": [
    {
      "type": "app",
      "name": "Firefox",
      "icon": "firefox"
    },
    {
      "type": "app",
      "name": "Terminal",
      "icon": "utilities-terminal"
    },
    {
      "type": "spacer"
    },
    {
      "type": "folder",
      "name": "Games",
      "icon": "applications-games",
      "apps": [
        {"type": "app", "name": "Steam"},
        {"type": "app", "name": "Lutris"}
      ]
    }
  ]
}
```

### App entry types

**Simple app:**
```json
{"type": "app", "name": "Firefox", "icon": "firefox"}
```

**App with custom exec:**
```json
{"type": "app", "name": "My App", "exec": "/path/to/app", "args": "--flag"}
```

**App from desktop file:**
```json
{"type": "app", "name": "Firefox", "desktop_file": "/usr/share/applications/firefox.desktop"}
```

**Custom icon path:**
```json
{"type": "app", "name": "My App", "icon": "app-icon", "custom_icon_path": "/home/user/icons/myapp.png"}
```

**Spacer:**
```json
{"type": "spacer"}
```

**Folder:**
```json
{
  "type": "folder",
  "name": "Utilities",
  "icon": "utilities-terminal",
  "show_miniatures": true,
  "apps": [
    {"type": "app", "name": "Terminal"},
    {"type": "app", "name": "File Manager"}
  ]
}
```

## Planned features

### High priority

- [ ] Right-click context menu (quit, reload config, edit)
- [ ] Clippy CI pipeline
- [ ] Better error handling (less unwrap())

see [full list of planned features](https://github.com/Kodeekk/rusty-dock/TODO.md)