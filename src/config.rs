use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono;

// ── New config fields for tasks ──────────────────────────────────────────────

fn default_animation_duration() -> u64 { 200 }
fn default_true() -> bool { true }
fn default_icon_zoom() -> f64 { 1.3 }
fn default_hide_delay() -> u64 { 500 }
fn default_smart_trigger_delay() -> u64 { 1000 }
fn default_smart_y_threshold() -> f64 { 6.0 }
fn default_folder_columns() -> u32 { 3 }
fn default_folder_icon_size() -> u32 { 48 }

// ── AppEntry ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppEntry {
    #[serde(rename = "app")]
    App {
        name: String,
        #[serde(default)]
        exec: String,
        #[serde(default)]
        args: String,
        #[serde(default)]
        icon: String,
        #[serde(default)]
        custom_icon_path: Option<String>,
        #[serde(default)]
        desktop_file: Option<String>,
    },
    #[serde(rename = "spacer")]
    Spacer,
    /// A folder groups multiple apps into a popup panel.
    /// Nested folders inside a folder are ignored.
    #[serde(rename = "folder")]
    Folder {
        name: String,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        apps: Vec<AppEntry>,
        #[serde(default)]
        show_miniatures: bool,
    },
}

impl AppEntry {
    pub fn new(name: String) -> Self {
        AppEntry::App {
            name,
            exec: String::new(),
            args: String::new(),
            icon: String::new(),
            custom_icon_path: None,
            desktop_file: None,
        }
    }

    pub fn new_spacer() -> Self { AppEntry::Spacer }

    pub fn new_folder(name: String) -> Self {
        AppEntry::Folder { name, icon: None, apps: Vec::new(), show_miniatures: false }
    }

    pub fn is_spacer(&self) -> bool { matches!(self, AppEntry::Spacer) }
    pub fn is_folder(&self) -> bool { matches!(self, AppEntry::Folder { .. }) }

    pub fn name(&self) -> &str {
        match self {
            AppEntry::App { name, .. } => name,
            AppEntry::Spacer => "---Spacer---",
            AppEntry::Folder { name, .. } => name,
        }
    }

    pub fn name_mut(&mut self) -> Option<&mut String> {
        match self {
            AppEntry::App { name, .. } => Some(name),
            AppEntry::Folder { name, .. } => Some(name),
            AppEntry::Spacer => None,
        }
    }

    pub fn exec_mut(&mut self) -> Option<&mut String> {
        match self {
            AppEntry::App { exec, .. } => Some(exec),
            _ => None,
        }
    }

    pub fn args_mut(&mut self) -> Option<&mut String> {
        match self {
            AppEntry::App { args, .. } => Some(args),
            _ => None,
        }
    }

    pub fn icon_mut(&mut self) -> Option<&mut String> {
        match self {
            AppEntry::App { icon, .. } => Some(icon),
            _ => None,
        }
    }

    pub fn folder_icon_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            AppEntry::Folder { icon, .. } => Some(icon),
            _ => None,
        }
    }

    pub fn custom_icon_path_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            AppEntry::App { custom_icon_path, .. } => Some(custom_icon_path),
            _ => None,
        }
    }

    pub fn desktop_file_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            AppEntry::App { desktop_file, .. } => Some(desktop_file),
            _ => None,
        }
    }

    pub fn folder_apps_mut(&mut self) -> Option<&mut Vec<AppEntry>> {
        match self {
            AppEntry::Folder { apps, .. } => Some(apps),
            _ => None,
        }
    }

    pub fn folder_show_miniatures_mut(&mut self) -> Option<&mut bool> {
        match self {
            AppEntry::Folder { show_miniatures, .. } => Some(show_miniatures),
            _ => None,
        }
    }

    pub fn folder_apps(&self) -> Option<&Vec<AppEntry>> {
        match self {
            AppEntry::Folder { apps, .. } => Some(apps),
            _ => None,
        }
    }
}

// ── DockConfig ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockConfig {
    #[serde(default)]
    pub debug: bool,

    pub pinned_apps: Vec<AppEntry>,

    // Visual
    pub icon_size: u32,
    pub dock_height: u32,
    pub position: DockPosition,
    pub background_opacity: f64,
    pub border_radius: f64,
    pub icon_padding: u32,
    pub dock_margin: i32,

    // Animations
    #[serde(default = "default_animation_duration")]
    pub animation_duration_ms: u64,
    #[serde(default = "default_animation_duration")]
    pub launch_bounce_duration_ms: u64,
    #[serde(default = "default_true")]
    pub enable_animations: bool,
    #[serde(default = "default_icon_zoom")]
    pub icon_zoom_on_hover: f64,
    #[serde(default = "default_true")]
    pub enable_bounce_on_launch: bool,

    // Auto-hide
    #[serde(default = "default_true")]
    pub auto_hide: bool,
    #[serde(default = "default_hide_delay")]
    pub auto_hide_delay_ms: u64,

    // Smart hide (task 5): only trigger when cursor is stationary at very bottom
    #[serde(default = "default_true")]
    pub smart_hide: bool,
    /// Milliseconds the cursor must remain at the bottom edge before the dock appears.
    #[serde(default = "default_smart_trigger_delay")]
    pub smart_hide_trigger_delay_ms: u64,
    /// Max vertical movement (px) still considered "stationary".
    #[serde(default = "default_smart_y_threshold")]
    pub smart_hide_y_threshold: f64,

    // Behaviour
    #[serde(default = "default_true")]
    pub show_app_names: bool,
    #[serde(default = "default_true")]
    pub show_active_indicators: bool,
    #[serde(default)]
    pub active_indicator_style: ActiveIndicatorStyle,
    #[serde(default)]
    pub grayscale_inactive_icons: bool,

    // Folders
    #[serde(default = "default_folder_columns")]
    pub folder_popup_columns: u32,
    #[serde(default = "default_folder_icon_size")]
    pub folder_popup_icon_size: u32,
    #[serde(default = "default_true")]
    pub show_folder_miniatures: bool,

    // Debug
    #[serde(default)]
    pub show_smart_hide_zone: bool,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            debug: false,
            pinned_apps: vec![
                AppEntry::App {
                    name: "Firefox".to_string(),
                    exec: String::new(),
                    args: String::new(),
                    icon: "firefox".to_string(),
                    custom_icon_path: None,
                    desktop_file: None,
                },
                AppEntry::App {
                    name: "Files".to_string(),
                    exec: String::new(),
                    args: String::new(),
                    icon: "system-file-manager".to_string(),
                    custom_icon_path: None,
                    desktop_file: None,
                },
                AppEntry::App {
                    name: "Terminal".to_string(),
                    exec: String::new(),
                    args: String::new(),
                    icon: "utilities-terminal".to_string(),
                    custom_icon_path: None,
                    desktop_file: None,
                },
            ],
            icon_size: 48,
            dock_height: 64,
            position: DockPosition::Bottom,
            background_opacity: 0.85,
            border_radius: 16.0,
            icon_padding: 8,
            dock_margin: 10,
            animation_duration_ms: 200,
            launch_bounce_duration_ms: 500,
            enable_animations: true,
            icon_zoom_on_hover: 1.3,
            enable_bounce_on_launch: true,
            auto_hide: true,
            auto_hide_delay_ms: 500,
            smart_hide: true,
            smart_hide_trigger_delay_ms: 1000,
            smart_hide_y_threshold: 6.0,
            show_app_names: true,
            show_active_indicators: true,
            folder_popup_columns: 3,
            folder_popup_icon_size: 48,
            show_folder_miniatures: true,
            show_smart_hide_zone: false,
            active_indicator_style: ActiveIndicatorStyle::Dot,
            grayscale_inactive_icons: false,
        }
    }
}

// ── DockPosition ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Top, Bottom, Left, Right,
}

// ── Active Indicator Style ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActiveIndicatorStyle {
    #[default]
    Dot,
    Underline,
    Border,
    Glow,
}

// ── Load / Save ───────────────────────────────────────────────────────────────

impl DockConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let json_path = Self::config_path_json()?;

        // --- JSON (primary) ---
        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read config file: {}", e))?;
            let config: DockConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
            config.validate()
                .map_err(|e| format!("Config validation failed: {}", e))?;
            return Ok(config);
        }

        // --- TOML migration ---
        let toml_path = Self::config_path_toml()?;
        if toml_path.exists() {
            println!("Migrating config from TOML → JSON…");
            let content = std::fs::read_to_string(&toml_path)
                .map_err(|e| format!("Failed to read TOML config: {}", e))?;
            let config: DockConfig = toml::from_str(&content)
                .map_err(|e| format!("Failed to parse TOML config: {}", e))?;
            config.validate()
                .map_err(|e| format!("Config validation failed: {}", e))?;
            config.save()?;
            let bak = toml_path.with_extension("toml.bak");
            let _ = std::fs::rename(&toml_path, &bak);
            println!("Migration done. Old config backed up to {:?}", bak);
            return Ok(config);
        }

        // --- First run ---
        let default_config = Self::default();
        default_config.save()?;
        Ok(default_config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), String> {
        if self.icon_size == 0 {
            return Err("icon_size must be greater than 0".to_string());
        }
        if self.dock_height == 0 {
            return Err("dock_height must be greater than 0".to_string());
        }
        if self.background_opacity < 0.0 || self.background_opacity > 1.0 {
            return Err("background_opacity must be between 0.0 and 1.0".to_string());
        }
        if self.border_radius < 0.0 {
            return Err("border_radius cannot be negative".to_string());
        }
        if self.animation_duration_ms == 0 {
            return Err("animation_duration_ms must be greater than 0".to_string());
        }
        if self.icon_zoom_on_hover < 1.0 {
            return Err("icon_zoom_on_hover must be at least 1.0".to_string());
        }
        Ok(())
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self.validate() {
            return Err(format!("Config validation failed: {}", e).into());
        }
        let path = Self::config_path_json()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Export config to a custom path (for backup/sharing)
    pub fn export_to_path(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Import config from a custom path
    pub fn import_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        let config: DockConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
        config.validate()
            .map_err(|e| format!("Config validation failed: {}", e))?;
        Ok(config)
    }

    /// Export config to default location with backup
    pub fn export_backup(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut backup_path = Self::config_path_json()?;
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        backup_path.set_file_name(format!("config_backup_{}.json", timestamp));
        self.export_to_path(&backup_path)?;
        Ok(backup_path)
    }

    fn config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut p = dirs::config_dir().ok_or("Could not find config directory")?;
        p.push("rusty-dock");
        Ok(p)
    }

    pub fn config_path_json() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    fn config_path_toml() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::config_dir()?.join("config.toml"))
    }
}