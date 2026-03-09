use freedesktop_desktop_entry::{default_paths, DesktopEntry, Iter};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use crate::config_window_iced::run_config_gui;
use crate::config::AppEntry as ConfigAppEntry;

// ── Entry (runtime representation) ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub exec: String,
    pub args: String,
    pub icon: Option<String>,
    pub desktop_file: Option<PathBuf>,
    pub is_active: bool,
    pub is_special: bool,
    pub is_spacer: bool,
    pub is_folder: bool,
    pub show_miniatures: bool,
    /// Apps inside a folder (only populated when is_folder = true)
    pub folder_entries: Vec<FolderEntry>,
}

#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub exec: String,
    pub args: String,
    pub icon: Option<String>,
    pub is_active: bool,
}

// ── AppLauncher ───────────────────────────────────────────────────────────────

pub struct AppLauncher {
    pub entries: Vec<Entry>,
    entry_map: HashMap<String, Entry>,
    pub debug: bool,
}

impl AppLauncher {
    pub fn new(pinned_apps: Vec<ConfigAppEntry>, debug: bool) -> Self {
        let mut launcher = Self { entries: Vec::new(), entry_map: HashMap::new(), debug };
        launcher.discover_apps();
        launcher.filter_pinned_apps(pinned_apps);
        launcher.add_config_button();
        launcher
    }

    fn log(&self, msg: &str) {
        if self.debug { println!("{}", msg); }
    }

    // ── Discovery ────────────────────────────────────────────────────────────

    fn discover_apps(&mut self) {
        self.log("Discovering applications…");
        for path in Iter::new(default_paths()) {
            if let Ok(bytes) = std::fs::read_to_string(&path) {
                if let Ok(entry) = DesktopEntry::from_str(&path, &bytes, None::<&[&str]>) {
                    if entry.no_display() { continue; }
                    let name = entry.name(&[] as &[&str]).map(|s| s.to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    let exec = entry.exec().map(|s| s.to_string()).unwrap_or_default();
                    if exec.is_empty() { continue; }
                    let icon = entry.icon().map(|s| s.to_string());
                    let app = Entry {
                        name: name.clone(), exec, args: String::new(), icon,
                        desktop_file: Some(path.to_path_buf()),
                        is_active: false, is_special: false,
                        is_spacer: false, is_folder: false,
                        show_miniatures: false,
                        folder_entries: vec![],
                    };
                    self.entry_map.insert(name.to_lowercase(), app);
                }
            }
        }
        if self.debug { println!("Found {} applications", self.entry_map.len()); }
    }

    // ── Pinned app resolution ─────────────────────────────────────────────────

    fn filter_pinned_apps(&mut self, pinned: Vec<ConfigAppEntry>) {
        for app_config in pinned {
            match app_config {
                ConfigAppEntry::Spacer => {
                    self.entries.push(Entry {
                        name: "Spacer".to_string(), exec: String::new(), args: String::new(),
                        icon: None, desktop_file: None,
                        is_active: false, is_special: false,
                        is_spacer: true, is_folder: false,
                        show_miniatures: false,
                        folder_entries: vec![],
                    });
                }

                ConfigAppEntry::Folder { name, icon, apps, show_miniatures } => {
                    let folder_entries = apps.iter()
                        .filter_map(|a| self.resolve_config_app_to_folder_entry(a))
                        .collect();
                    self.entries.push(Entry {
                        name, exec: String::new(), args: String::new(),
                        icon,
                        desktop_file: None,
                        is_active: false, is_special: false,
                        is_spacer: false, is_folder: true,
                        show_miniatures,
                        folder_entries,
                    });
                }

                ConfigAppEntry::App { name, exec, args, icon, custom_icon_path, desktop_file } => {
                    let entry = self.resolve_app(&name, &exec, &args, &icon, &custom_icon_path, &desktop_file);
                    self.entries.push(entry);
                }
            }
        }
        self.entries.dedup_by(|a, b| !a.is_spacer && !b.is_spacer && a.name == b.name);
    }

    fn resolve_config_app_to_folder_entry(&self, config: &ConfigAppEntry) -> Option<FolderEntry> {
        match config {
            ConfigAppEntry::App { name, exec, args, icon, custom_icon_path, desktop_file } => {
                let mut fe = FolderEntry {
                    name: name.clone(),
                    exec: exec.clone(),
                    args: args.clone(),
                    icon: if let Some(ci) = custom_icon_path { Some(ci.clone()) }
                    else if !icon.is_empty() { Some(icon.clone()) }
                    else { None },
                    is_active: false,
                };

                // Try desktop file first
                if let Some(df) = desktop_file {
                    if let Some(e) = self.load_from_desktop_file(df) {
                        if fe.exec.is_empty() { fe.exec = e.exec; }
                        if fe.icon.is_none() { fe.icon = e.icon; }
                        if fe.name.is_empty() || fe.name == "New App" { fe.name = e.name; }
                    }
                }

                // Try discovered apps as fallback
                if fe.exec.is_empty() {
                    if let Some(discovered) = self.entry_map.get(&name.to_lowercase()) {
                        fe.exec = discovered.exec.clone();
                        if fe.icon.is_none() { fe.icon = discovered.icon.clone(); }
                    }
                }

                Some(fe)
            }
            _ => None, // ignore spacers/folders inside folders
        }
    }

    fn resolve_app(
        &self,
        name: &str, exec: &str, args: &str, icon: &str,
        custom_icon_path: &Option<String>,
        desktop_file: &Option<String>,
    ) -> Entry {
        let make_icon = |custom: &Option<String>, fallback: &str| -> Option<String> {
            if let Some(ci) = custom { Some(ci.clone()) }
            else if !fallback.is_empty() { Some(fallback.to_string()) }
            else { None }
        };

        // Priority 1: desktop file
        if let Some(df) = desktop_file {
            if let Some(mut e) = self.load_from_desktop_file(df) {
                if !exec.is_empty() { e.exec = exec.to_string(); }
                if !args.is_empty() { e.args = args.to_string(); }
                let icon_override = make_icon(custom_icon_path, icon);
                if icon_override.is_some() { e.icon = icon_override; }
                if !name.is_empty() && name != "New App" { e.name = name.to_string(); }
                return e;
            }
        }

        // Priority 2: explicit exec
        if !exec.is_empty() {
            return Entry {
                name: name.to_string(), exec: exec.to_string(), args: args.to_string(),
                icon: make_icon(custom_icon_path, icon),
                desktop_file: None,
                is_active: false, is_special: false,
                is_spacer: false, is_folder: false,
                show_miniatures: false,
                folder_entries: vec![],
            };
        }

        // Priority 3: discovery
        let key = name.to_lowercase();
        if let Some(app) = self.entry_map.get(&key) {
            let mut cloned = app.clone();
            let icon_override = make_icon(custom_icon_path, icon);
            if icon_override.is_some() { cloned.icon = icon_override; }
            if !args.is_empty() { cloned.args = args.to_string(); }
            return cloned;
        }
        for (k, app) in &self.entry_map {
            if k.contains(&key) || key.contains(k.as_str()) {
                let mut cloned = app.clone();
                let icon_override = make_icon(custom_icon_path, icon);
                if icon_override.is_some() { cloned.icon = icon_override; }
                if !args.is_empty() { cloned.args = args.to_string(); }
                return cloned;
            }
        }

        // Placeholder
        Entry {
            name: name.to_string(), exec: String::new(), args: args.to_string(),
            icon: make_icon(custom_icon_path, icon),
            desktop_file: None,
            is_active: false, is_special: false,
            is_spacer: false, is_folder: false,
            show_miniatures: false,
            folder_entries: vec![],
        }
    }

    fn load_from_desktop_file(&self, desktop_path: &str) -> Option<Entry> {
        let path = PathBuf::from(desktop_path);
        if !path.exists() { return None; }
        let bytes = std::fs::read_to_string(&path).ok()?;
        let entry = DesktopEntry::from_str(&path, &bytes, None::<&[&str]>).ok()?;
        Some(Entry {
            name: entry.name(&[] as &[&str])?.to_string(),
            exec: entry.exec()?.to_string(),
            args: String::new(),
            icon: entry.icon().map(|s| s.to_string()),
            desktop_file: Some(path),
            is_active: false, is_special: false,
            is_spacer: false, is_folder: false,
            show_miniatures: false,
            folder_entries: vec![],
        })
    }

    fn add_config_button(&mut self) {
        self.entries.push(Entry {
            name: "Settings".to_string(),
            exec: "internal:config".to_string(),
            args: String::new(),
            icon: None, desktop_file: None,
            is_active: false, is_special: true,
            is_spacer: false, is_folder: false,
            show_miniatures: false,
            folder_entries: vec![],
        });
    }

    // ── Launch ───────────────────────────────────────────────────────────────

    /// Returns `(reload_config, spawned_pid)`.
    pub fn launch_app(&mut self, index: usize) -> (bool, Option<u32>) {
        let entry = match self.entries.get(index) {
            Some(e) => e,
            None => return (false, None),
        };

        if entry.is_spacer || entry.is_folder { return (false, None); }

        if entry.is_special {
            run_config_gui();
            return (true, None);
        }

        let name = entry.name.clone();
        let raw_exec = entry.exec.clone();

        if raw_exec.is_empty() {
            eprintln!("No exec for: {}", name);
            return (false, None);
        }

        let exec = self.clean_exec(&raw_exec);
        let mut parts = self.parse_exec(&exec);
        if parts.is_empty() { return (false, None); }

        if !entry.args.is_empty() {
            parts.extend(self.parse_exec(&entry.args));
        }

        match Command::new(&parts[0]).args(&parts[1..]).spawn() {
            Ok(child) => {
                let pid = child.id();
                if let Some(e) = self.entries.get_mut(index) { e.is_active = true; }
                (false, Some(pid))
            }
            Err(e) => { eprintln!("Failed to launch {}: {}", name, e); (false, None) }
        }
    }

    /// Launch an app inside a folder. Returns the spawned PID.
    pub fn launch_folder_app(
        &mut self,
        folder_index: usize,
        app_index: usize,
    ) -> Option<u32> {
        let (exec, args, name) = {
            let folder = self.entries.get(folder_index)?;
            let fe = folder.folder_entries.get(app_index)?;
            (fe.exec.clone(), fe.args.clone(), fe.name.clone())
        };

        if exec.is_empty() {
            eprintln!("No exec for folder app: {}", name);
            return None;
        }

        let exec = self.clean_exec(&exec);
        let mut parts = self.parse_exec(&exec);
        if parts.is_empty() { return None; }

        if !args.is_empty() {
            parts.extend(self.parse_exec(&args));
        }

        match Command::new(&parts[0]).args(&parts[1..]).spawn() {
            Ok(child) => {
                let pid = child.id();
                if let Some(f) = self.entries.get_mut(folder_index) {
                    if let Some(fe) = f.folder_entries.get_mut(app_index) {
                        fe.is_active = true;
                    }
                }
                Some(pid)
            }
            Err(e) => { eprintln!("Failed to launch {}: {}", name, e); None }
        }
    }

    pub fn reorder_app(&mut self, from: usize, to: usize) {
        if from >= self.entries.len() || to >= self.entries.len() || from == to { return; }
        if self.entries[from].is_special || self.entries[to].is_special { return; }
        let app = self.entries.remove(from);
        self.entries.insert(to, app);
    }

    fn clean_exec(&self, exec: &str) -> String {
        exec.split_whitespace()
            .filter(|s| !s.starts_with('%'))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn parse_exec(&self, exec: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        for c in exec.chars() {
            match c {
                '"' => in_quotes = !in_quotes,
                ' ' if !in_quotes => {
                    if !current.is_empty() { parts.push(current.clone()); current.clear(); }
                }
                _ => current.push(c),
            }
        }
        if !current.is_empty() { parts.push(current); }
        parts
    }
}