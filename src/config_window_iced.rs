use iced::alignment::Horizontal;
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length, Application, Settings, Theme, Command, Subscription, executor, keyboard};
use std::path::PathBuf;
use std::process::Command as SysCommand;
use freedesktop_desktop_entry::{default_paths, DesktopEntry, Iter};

use crate::config::{AppEntry, DockConfig, DockPosition, ActiveIndicatorStyle};

pub fn run_config_gui() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = SysCommand::new(exe).arg("--config").spawn();
    }
}

pub fn run_config_window_blocking() {
    let mut settings = Settings::default();
    settings.window.size = iced::Size::new(750.0, 850.0);
    let _ = ConfigApp::run(settings);
}

pub fn reload_signal_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut p = std::env::temp_dir();
    p.push("rusty-dock-reload.signal");
    Ok(p)
}

fn create_reload_signal() {
    if let Ok(p) = reload_signal_path() { let _ = std::fs::write(p, "reload"); }
}

#[derive(Debug, Clone, Copy)]
struct M3Palette {
    primary: Color,
    on_primary: Color,
    surface: Color,
    on_surface: Color,
    on_surface_variant: Color,
}

impl Default for M3Palette {
    fn default() -> Self {
        Self {
            primary: Color::from_rgba8(208, 188, 255, 1.0),
            on_primary: Color::from_rgba8(56, 30, 114, 1.0),
            surface: Color::from_rgba8(28, 27, 31, 1.0),
            on_surface: Color::from_rgba8(230, 225, 229, 1.0),
            on_surface_variant: Color::from_rgba8(202, 196, 208, 1.0),
        }
    }
}

#[derive(Clone)]
struct DesktopFileEntry { name: String, path: String }

#[derive(Debug, Clone, Copy, PartialEq)]
enum PickerMode { Icons, Executables, Any }

#[derive(Clone, Debug)]
enum PickerTarget {
    DockEntry(usize, DockEntryField),
    FolderApp(usize, usize, DockEntryField),
}

#[derive(Clone, Debug, PartialEq)]
enum DockEntryField { Icon, Exec, Desktop }

#[derive(Clone, Debug, Copy, PartialEq)]
enum DragId {
    Top(usize),
    Folder(usize, usize),
}

#[derive(Clone, Debug)]
enum Message {
    Save, Reset, None,
    IconSizeV(u32),
    DockHeightV(u32),
    IconPaddingV(u32),
    BorderRadiusV(f32),
    BackgroundOpacityV(f32),
    DockMarginV(i32),
    Position(DockPosition),
    EnableAnimations(bool),
    DurationV(u64),
    BounceDurationV(u64),
    HoverZoomV(f64),
    BounceOnLaunch(bool),
    AutoHide(bool),
    HideDelayV(u64),
    SmartHide(bool),
    SmartDelayV(u64),
    SmartYThresholdV(f64),
    ShowNames(bool),
    ShowIndicators(bool),
    IndicatorStyle(ActiveIndicatorStyle),
    GrayscaleInactive(bool),
    Debug(bool),
    ShowSmartHideZone(bool),
    FolderColumnsV(u32),
    FolderIconSizeV(u32),
    FolderMiniatures(usize, bool),
    // Pinned list actions
    AddApp, AddSpacer, AddFolder,
    Remove(usize), Edit(usize),
    Drag(DragId), Drop(DragId),
    DoneEditing,
    AddAppToFolder(usize),
    FaRemove(usize, usize),
    EditFolderApp(usize, usize),
    AppName(usize, String),
    AppExec(usize, String),
    AppArgs(usize, String),
    AppExecBrowse(usize),
    AppIconTheme(usize, String),
    AppCustomIcon(usize),
    AppDesktopList(usize),
    AppDesktopFile(usize),
    FolderIconPath(usize, String),
    FaName(usize, usize, String),
    FaExec(usize, usize, String),
    FaArgs(usize, usize, String),
    FaExecBrowse(usize, usize),
    FaIconTheme(usize, usize, String),
    FaCustomIcon(usize, usize),
    FaDesktopList(usize, usize),
    FaDesktopFile(usize, usize),
    DesktopSearch(String),
    DesktopPicked(String),
    HideDesktopPicker,
    OpenPicker(PickerTarget, PickerMode),
    PickerSearch(String),
    PickerNavigate(PathBuf),
    PickerPicked(String),
    HidePicker,
    PickerSubmit,
    PickerToggleHidden(bool),
    KeyboardEvent(iced::keyboard::Event),
}

struct FilePickerState {
    title: String,
    mode: PickerMode,
    current_dir: PathBuf,
    search: String,
    entries: Vec<(String, bool)>,
    target: PickerTarget,
    show_hidden: bool,
}

struct ConfigApp {
    palette: M3Palette,
    config: DockConfig,
    status_msg: String,
    selected_app_index: Option<usize>,
    selected_folder_app: Option<(usize, usize)>,
    desktop_files: Vec<DesktopFileEntry>,
    desktop_search: String,
    show_desktop_picker: Option<PickerTarget>,
    file_picker: Option<FilePickerState>,
    dragging_id: Option<DragId>,
}

impl Application for ConfigApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut app = Self {
            palette: M3Palette::default(),
            config: DockConfig::load().unwrap_or_default(),
            status_msg: String::new(),
            selected_app_index: None,
            selected_folder_app: None,
            desktop_files: vec![],
            desktop_search: String::new(),
            show_desktop_picker: None,
            file_picker: None,
            dragging_id: None,
        };
        app.load_desktop_files();
        (app, Command::none())
    }

    fn title(&self) -> String { "Rusty Dock — Settings".into() }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Save => {
                if let Err(e) = self.config.save() { self.status_msg = format!("Error: {}", e); }
                else { self.status_msg = "Saved ✓".into(); create_reload_signal(); }
            }
            Message::Reset => { self.config = DockConfig::default(); self.status_msg = "Reset (not saved)".into(); }
            Message::IconSizeV(v) => self.config.icon_size = v,
            Message::DockHeightV(v) => self.config.dock_height = v,
            Message::IconPaddingV(v) => self.config.icon_padding = v,
            Message::BorderRadiusV(v) => self.config.border_radius = v as f64,
            Message::BackgroundOpacityV(v) => self.config.background_opacity = v as f64,
            Message::DockMarginV(v) => self.config.dock_margin = v,
            Message::Position(p) => self.config.position = p,
            Message::EnableAnimations(b) => self.config.enable_animations = b,
            Message::DurationV(v) => self.config.animation_duration_ms = v,
            Message::BounceDurationV(v) => self.config.launch_bounce_duration_ms = v,
            Message::HoverZoomV(v) => self.config.icon_zoom_on_hover = v as f64,
            Message::BounceOnLaunch(b) => self.config.enable_bounce_on_launch = b,
            Message::AutoHide(b) => self.config.auto_hide = b,
            Message::HideDelayV(v) => self.config.auto_hide_delay_ms = v,
            Message::SmartHide(b) => self.config.smart_hide = b,
            Message::SmartDelayV(v) => self.config.smart_hide_trigger_delay_ms = v,
            Message::SmartYThresholdV(v) => self.config.smart_hide_y_threshold = v as f64,
            Message::ShowNames(b) => self.config.show_app_names = b,
            Message::ShowIndicators(b) => self.config.show_active_indicators = b,
            Message::IndicatorStyle(style) => self.config.active_indicator_style = style,
            Message::GrayscaleInactive(b) => self.config.grayscale_inactive_icons = b,
            Message::Debug(b) => self.config.debug = b,
            Message::ShowSmartHideZone(b) => self.config.show_smart_hide_zone = b,
            Message::FolderColumnsV(v) => self.config.folder_popup_columns = v,
            Message::FolderIconSizeV(v) => self.config.folder_popup_icon_size = v,
            Message::FolderMiniatures(i, b) => {
                if let Some(AppEntry::Folder { show_miniatures, .. }) = self.config.pinned_apps.get_mut(i) {
                    *show_miniatures = b;
                }
            }
            Message::AddApp => self.config.pinned_apps.push(AppEntry::new("New App".into())),
            Message::AddSpacer => self.config.pinned_apps.push(AppEntry::new_spacer()),
            Message::AddFolder => self.config.pinned_apps.push(AppEntry::new_folder("New Folder".into())),
            Message::Remove(i) => { self.config.pinned_apps.remove(i); self.selected_app_index = None; },
            Message::Edit(i) => {
                if self.selected_app_index == Some(i) { self.selected_app_index = None; }
                else { self.selected_app_index = Some(i); self.selected_folder_app = None; }
            }
            Message::Drag(id) => self.dragging_id = Some(id),
            Message::Drop(target) => {
                if let Some(source) = self.dragging_id.take() {
                    self.handle_drag_drop(source, target);
                }
            }
            Message::DoneEditing => { self.selected_app_index = None; self.selected_folder_app = None; },
            Message::AddAppToFolder(i) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(i) { apps.push(AppEntry::new("New App".into())); },
            Message::FaRemove(fi, ai) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) { apps.remove(ai); },
            Message::EditFolderApp(fi, ai) => {
                if self.selected_folder_app == Some((fi, ai)) { self.selected_folder_app = None; }
                else { self.selected_folder_app = Some((fi, ai)); }
            }
            Message::AppName(i, v) => if let Some(n) = self.config.pinned_apps.get_mut(i).and_then(|e| e.name_mut()) { *n = v; },
            Message::AppExec(i, v) => if let Some(e) = self.config.pinned_apps.get_mut(i).and_then(|e| e.exec_mut()) { *e = v; },
            Message::AppArgs(i, v) => if let Some(a) = self.config.pinned_apps.get_mut(i).and_then(|e| e.args_mut()) { *a = v; },
            Message::AppExecBrowse(i) => {
                return self.update(Message::OpenPicker(PickerTarget::DockEntry(i, DockEntryField::Exec), PickerMode::Executables));
            }
            Message::AppIconTheme(i, v) => if let Some(ic) = self.config.pinned_apps.get_mut(i).and_then(|e| e.icon_mut()) { *ic = v; },
            Message::AppCustomIcon(i) => {
                return self.update(Message::OpenPicker(PickerTarget::DockEntry(i, DockEntryField::Icon), PickerMode::Icons));
            }
            Message::AppDesktopList(i) => {
                self.show_desktop_picker = Some(PickerTarget::DockEntry(i, DockEntryField::Desktop));
                self.desktop_search.clear();
            },
            Message::AppDesktopFile(i) => {
                return self.update(Message::OpenPicker(PickerTarget::DockEntry(i, DockEntryField::Desktop), PickerMode::Any));
            }
            Message::FolderIconPath(i, v) => if let Some(ic) = self.config.pinned_apps.get_mut(i).and_then(|e| e.folder_icon_mut()) { *ic = if v.is_empty() { None } else { Some(v) }; },
            Message::FaName(fi, ai, v) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) { if let Some(n) = apps.get_mut(ai).and_then(|e| e.name_mut()) { *n = v; } },
            Message::FaExec(fi, ai, v) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) { if let Some(e) = apps.get_mut(ai).and_then(|e| e.exec_mut()) { *e = v; } },
            Message::FaArgs(fi, ai, v) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) { if let Some(a) = apps.get_mut(ai).and_then(|e| e.args_mut()) { *a = v; } },
            Message::FaExecBrowse(fi, ai) => {
                return self.update(Message::OpenPicker(PickerTarget::FolderApp(fi, ai, DockEntryField::Exec), PickerMode::Executables));
            }
            Message::FaIconTheme(fi, ai, v) => if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) { if let Some(ic) = apps.get_mut(ai).and_then(|e| e.icon_mut()) { *ic = v; } },
            Message::FaCustomIcon(fi, ai) => {
                return self.update(Message::OpenPicker(PickerTarget::FolderApp(fi, ai, DockEntryField::Icon), PickerMode::Icons));
            }
            Message::FaDesktopList(fi, ai) => {
                self.show_desktop_picker = Some(PickerTarget::FolderApp(fi, ai, DockEntryField::Desktop));
                self.desktop_search.clear();
            },
            Message::FaDesktopFile(fi, ai) => {
                return self.update(Message::OpenPicker(PickerTarget::FolderApp(fi, ai, DockEntryField::Desktop), PickerMode::Any));
            }
            Message::DesktopSearch(s) => self.desktop_search = s,
            Message::DesktopPicked(p) => {
                if let Some(target) = self.show_desktop_picker.take() {
                    self.apply_picker_result(target, p);
                }
            }
            Message::HideDesktopPicker => self.show_desktop_picker = None,
            Message::OpenPicker(target, mode) => {
                let start_dir = match mode {
                    PickerMode::Icons => PathBuf::from("/usr/share/icons"),
                    PickerMode::Executables => PathBuf::from("/usr/bin"),
                    PickerMode::Any => PathBuf::from("/"),
                };
                let title = match mode {
                    PickerMode::Icons => "Select Icon",
                    PickerMode::Executables => "Select Executable",
                    PickerMode::Any => "Select File",
                };
                self.file_picker = Some(FilePickerState {
                    title: title.into(),
                    mode,
                    current_dir: start_dir,
                    search: String::new(),
                    entries: vec![],
                    target,
                    show_hidden: false,
                });
                self.refresh_picker();
            }
            Message::PickerSearch(s) => {
                if let Some(fp) = &mut self.file_picker {
                    fp.search = s;
                }
            }
            Message::PickerNavigate(path) => {
                if let Some(fp) = &mut self.file_picker {
                    fp.current_dir = path;
                    self.refresh_picker();
                }
            }
            Message::PickerPicked(p) => {
                if let Some(fp) = self.file_picker.take() {
                    self.apply_picker_result(fp.target, p);
                }
            }
            Message::HidePicker => self.file_picker = None,
            Message::PickerSubmit => {
                if let Some(fp) = &mut self.file_picker {
                    let search_lc = fp.search.to_lowercase();
                    let matched_dir = fp.entries.iter()
                        .filter(|(name, is_dir)| *is_dir && (search_lc.is_empty() || name.to_lowercase().contains(&search_lc)))
                        .next()
                        .map(|(name, _)| fp.current_dir.join(name));
                    if let Some(path) = matched_dir {
                        fp.current_dir = path;
                        fp.search.clear();
                        self.refresh_picker();
                    }
                }
            }
            Message::PickerToggleHidden(b) => {
                if let Some(fp) = &mut self.file_picker {
                    fp.show_hidden = b;
                    self.refresh_picker();
                }
            }
            Message::KeyboardEvent(event) => {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    if let keyboard::Key::Character(ref c) = key {
                        if c == "s" && modifiers.command() {
                            return self.update(Message::Save);
                        }
                    }
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        if self.file_picker.is_some() {
                            return self.update(Message::HidePicker);
                        }
                        if self.show_desktop_picker.is_some() {
                            return self.update(Message::HideDesktopPicker);
                        }
                    }
                }
            }
            Message::None => {}
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyboardEvent(keyboard::Event::KeyPressed { key, modifiers, location: keyboard::Location::Standard, text: None }))
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let header = row![
            text("Rusty Dock — Settings").size(28).style(self.palette.primary),
            Space::with_width(Length::Fill),
            button(text("Save").horizontal_alignment(Horizontal::Center))
                .padding([10, 24])
                .on_press(Message::Save),
            button(text("Reset").horizontal_alignment(Horizontal::Center))
                .padding([10, 24])
                .on_press(Message::Reset),
        ]
        .spacing(16)
        .align_items(Alignment::Center);

        let content = column![
            header,
            Space::with_height(16),
            scrollable(
                column![
                    self.view_appearance(),
                    self.view_animations(),
                    self.view_auto_hide(),
                    self.view_smart_hide(),
                    self.view_behaviour(),
                    self.view_debug(),
                    self.view_folders(),
                    self.view_pinned_apps(),
                ].spacing(32)
            ),
            Space::with_height(16),
            row![
                text(&self.status_msg).size(16),
                Space::with_width(Length::Fill),
                text("Ctrl+S to save").size(14).style(self.palette.on_surface_variant),
            ].align_items(Alignment::Center),
        ].padding(24);

        let root = container(content)
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(_) = &self.show_desktop_picker {
            container(self.view_desktop_picker())
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
        } else if let Some(_) = &self.file_picker {
            container(self.view_picker())
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
        } else {
            root.into()
        }
    }

    fn theme(&self) -> Theme { Theme::Dark }
}

impl ConfigApp {
    fn refresh_picker(&mut self) {
        if let Some(fp) = &mut self.file_picker {
            fp.entries.clear();
            let Ok(rd) = std::fs::read_dir(&fp.current_dir) else { return };
            let mut dirs = vec![];
            let mut files = vec![];
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !fp.show_hidden && name.starts_with('.') { continue; }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    dirs.push((name, true));
                } else {
                    let keep = match fp.mode {
                        PickerMode::Icons => {
                            let lower = name.to_lowercase();
                            lower.ends_with(".png") || lower.ends_with(".svg")
                                || lower.ends_with(".xpm") || lower.ends_with(".ico")
                        }
                        PickerMode::Executables => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                entry.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
                            }
                            #[cfg(not(unix))]
                            true
                        }
                        PickerMode::Any => true,
                    };
                    if keep { files.push((name, false)); }
                }
            }
            dirs.sort_by(|a, b| a.0.cmp(&b.0));
            files.sort_by(|a, b| a.0.cmp(&b.0));
            fp.entries.extend(dirs);
            fp.entries.extend(files);
        }
    }

    fn apply_picker_result(&mut self, target: PickerTarget, value: String) {
        match target {
            PickerTarget::DockEntry(idx, field) => {
                if let Some(entry) = self.config.pinned_apps.get_mut(idx) {
                    match field {
                        DockEntryField::Icon => {
                            match entry {
                                AppEntry::App { custom_icon_path, .. } => *custom_icon_path = Some(value),
                                AppEntry::Folder { icon, .. } => *icon = Some(value),
                                _ => {}
                            }
                        }
                        DockEntryField::Exec => {
                            if let Some(e) = entry.exec_mut() { *e = value; }
                        }
                        DockEntryField::Desktop => {
                            if let Some(df) = entry.desktop_file_mut() { *df = Some(value); }
                        }
                    }
                }
            }
            PickerTarget::FolderApp(fi, ai, field) => {
                if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) {
                    if let Some(entry) = apps.get_mut(ai) {
                        match field {
                            DockEntryField::Icon => {
                                if let Some(ic) = entry.custom_icon_path_mut() { *ic = Some(value); }
                            }
                            DockEntryField::Exec => {
                                if let Some(e) = entry.exec_mut() { *e = value; }
                            }
                            DockEntryField::Desktop => {
                                if let Some(df) = entry.desktop_file_mut() { *df = Some(value); }
                            }
                        }
                    }
                }
            }
        }
    }

    fn load_desktop_files(&mut self) {
        self.desktop_files.clear();
        for path in Iter::new(default_paths()) {
            if let Ok(bytes) = std::fs::read_to_string(&path) {
                if let Ok(entry) = DesktopEntry::from_str(&path, &bytes, None::<&[&str]>) {
                    if entry.no_display() { continue; }
                    if let Some(name) = entry.name(&[] as &[&str]) {
                        self.desktop_files.push(DesktopFileEntry { name: name.to_string(), path: path.to_string_lossy().to_string() });
                    }
                }
            }
        }
        self.desktop_files.sort_by(|a, b| a.name.cmp(&b.name));
    }

    fn handle_drag_drop(&mut self, source: DragId, target: DragId) {
        if source == target { return; }
        if let (DragId::Top(si), DragId::Folder(tf, _)) = (source, target) {
            if si == tf { return; }
        }
        let item = match source {
            DragId::Top(i) => {
                if i < self.config.pinned_apps.len() {
                    Some(self.config.pinned_apps.remove(i))
                } else { None }
            }
            DragId::Folder(fi, ai) => {
                if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(fi) {
                    if ai < apps.len() {
                        Some(apps.remove(ai))
                    } else { None }
                } else { None }
            }
        };
        if let Some(it) = item {
            match target {
                DragId::Top(ti) => {
                    let ti = ti.min(self.config.pinned_apps.len());
                    self.config.pinned_apps.insert(ti, it);
                }
                DragId::Folder(tf, af) => {
                    if let Some(AppEntry::Folder { apps, .. }) = self.config.pinned_apps.get_mut(tf) {
                        let af = af.min(apps.len());
                        apps.insert(af, it);
                    } else {
                        self.config.pinned_apps.push(it);
                    }
                }
            }
        }
    }

    fn view_appearance(&self) -> Element<'_, Message> {
        use iced::widget::slider;
        column![
            text("Appearance").size(18),
            row![text("Icon size:"), slider(16.0..=128.0, self.config.icon_size as f32, |v| Message::IconSizeV(v as u32)), text(format!("{}", self.config.icon_size))].spacing(12),
            row![text("Dock height:"), slider(32.0..=128.0, self.config.dock_height as f32, |v| Message::DockHeightV(v as u32)), text(format!("{}", self.config.dock_height))].spacing(12),
            row![text("Icon padding:"), slider(0.0..=32.0, self.config.icon_padding as f32, |v| Message::IconPaddingV(v as u32)), text(format!("{}", self.config.icon_padding))].spacing(12),
            row![text("Border radius:"), slider(0.0..=64.0, self.config.border_radius as f32, Message::BorderRadiusV), text(format!("{:.0}", self.config.border_radius))].spacing(12),
            row![text("Opacity:"), slider(0.1..=1.0, self.config.background_opacity as f32, Message::BackgroundOpacityV), text(format!("{:.2}", self.config.background_opacity))].spacing(12),
            row![text("Margin:"), text_input("", &self.config.dock_margin.to_string()).on_input(|s| if let Ok(v) = s.parse() { Message::DockMarginV(v) } else { Message::None }),].spacing(12),
            row![
                text("Position:").width(Length::Fixed(80.0)),
                button(text(format!("{:?}", self.config.position))).on_press(Message::Position(match self.config.position {
                    DockPosition::Bottom => DockPosition::Top,
                    DockPosition::Top => DockPosition::Left,
                    DockPosition::Left => DockPosition::Right,
                    DockPosition::Right => DockPosition::Bottom,
                })),
            ].spacing(12),
        ].spacing(16).into()
    }

    fn view_animations(&self) -> Element<'_, Message> {
        use iced::widget::slider;
        column![
            text("Animations").size(18),
            checkbox("Enable animations", self.config.enable_animations).on_toggle(Message::EnableAnimations),
            row![text("Duration:"), slider(50.0..=1000.0, self.config.animation_duration_ms as f32, |v| Message::DurationV(v as u64)), text(format!("{}ms", self.config.animation_duration_ms))].spacing(12),
            row![text("Bounce duration:"), slider(100.0..=1000.0, self.config.launch_bounce_duration_ms as f32, |v| Message::BounceDurationV(v as u64)), text(format!("{}ms", self.config.launch_bounce_duration_ms))].spacing(12),
            checkbox("Bounce on launch", self.config.enable_bounce_on_launch).on_toggle(Message::BounceOnLaunch),
            row![text("Hover zoom:"), slider(1.0..=2.0, self.config.icon_zoom_on_hover as f32, |v| Message::HoverZoomV(v as f64)), text(format!("{:.1}x", self.config.icon_zoom_on_hover))].spacing(12),
        ].spacing(16).into()
    }

    fn view_auto_hide(&self) -> Element<'_, Message> {
        use iced::widget::slider;
        column![
            text("Auto-hide").size(18),
            checkbox("Enable auto-hide", self.config.auto_hide).on_toggle(Message::AutoHide),
            row![text("Hide delay:"), slider(100.0..=2000.0, self.config.auto_hide_delay_ms as f32, |v| Message::HideDelayV(v as u64)), text(format!("{}ms", self.config.auto_hide_delay_ms))].spacing(12),
        ].spacing(16).into()
    }

    fn view_smart_hide(&self) -> Element<'_, Message> {
        use iced::widget::slider;
        column![
            text("Smart-hide").size(18),
            checkbox("Enable smart-hide", self.config.smart_hide).on_toggle(Message::SmartHide),
            row![text("Trigger delay:"), slider(500.0..=3000.0, self.config.smart_hide_trigger_delay_ms as f32, |v| Message::SmartDelayV(v as u64)), text(format!("{}ms", self.config.smart_hide_trigger_delay_ms))].spacing(12),
            row![text("Y threshold:"), slider(1.0..=20.0, self.config.smart_hide_y_threshold as f32, |v| Message::SmartYThresholdV(v as f64)), text(format!("{:.0}px", self.config.smart_hide_y_threshold))].spacing(12),
        ].spacing(16).into()
    }

    fn view_behaviour(&self) -> Element<'_, Message> {
        column![
            text("Behaviour").size(18),
            checkbox("Show app names (tooltips)", self.config.show_app_names).on_toggle(Message::ShowNames),
            checkbox("Show active indicators", self.config.show_active_indicators).on_toggle(Message::ShowIndicators),
            row![
                text("Indicator style:").width(Length::Fixed(120.0)),
                button(text(format!("{:?}", self.config.active_indicator_style))).on_press(Message::IndicatorStyle(match self.config.active_indicator_style {
                    ActiveIndicatorStyle::Dot => ActiveIndicatorStyle::Underline,
                    ActiveIndicatorStyle::Underline => ActiveIndicatorStyle::Border,
                    ActiveIndicatorStyle::Border => ActiveIndicatorStyle::Glow,
                    ActiveIndicatorStyle::Glow => ActiveIndicatorStyle::Dot,
                })),
            ].spacing(12),
            checkbox("Grayscale inactive icons", self.config.grayscale_inactive_icons).on_toggle(Message::GrayscaleInactive),
        ].spacing(16).into()
    }

    fn view_debug(&self) -> Element<'_, Message> {
        column![
            text("Debug").size(18),
            checkbox("Debug mode", self.config.debug).on_toggle(Message::Debug),
            checkbox("Show smart-hide zone", self.config.show_smart_hide_zone).on_toggle(Message::ShowSmartHideZone),
        ].spacing(16).into()
    }

    fn view_folders(&self) -> Element<'_, Message> {
        use iced::widget::slider;
        column![
            text("Folders").size(18),
            row![text("Columns:"), slider(1.0..=6.0, self.config.folder_popup_columns as f32, |v| Message::FolderColumnsV(v as u32)), text(format!("{}", self.config.folder_popup_columns))].spacing(12),
            row![text("Icon size:"), slider(32.0..=96.0, self.config.folder_popup_icon_size as f32, |v| Message::FolderIconSizeV(v as u32)), text(format!("{}", self.config.folder_popup_icon_size))].spacing(12),
        ].spacing(16).into()
    }

    fn view_pinned_apps(&self) -> Element<'_, Message> {
        let mut apps = column![].spacing(8);
        for (i, entry) in self.config.pinned_apps.iter().enumerate() {
            let row = row![
                text(entry.name()).width(Length::Fill),
                button(text("Edit")).on_press(Message::Edit(i)),
                button(text("Remove")).on_press(Message::Remove(i)),
            ].spacing(8);
            apps = apps.push(row);
        }
        let actions = row![
            button(text("+ App")).on_press(Message::AddApp),
            button(text("+ Folder")).on_press(Message::AddFolder),
            button(text("+ Spacer")).on_press(Message::AddSpacer),
        ].spacing(12);
        column![text("Pinned Apps").size(18), apps, actions].spacing(16).into()
    }

    fn view_desktop_picker(&self) -> Element<'_, Message> {
        let mut list = column![].spacing(4);
        let search_lc = self.desktop_search.to_lowercase();
        for entry in &self.desktop_files {
            if search_lc.is_empty() || entry.name.to_lowercase().contains(&search_lc) {
                let btn = button(text(&entry.name))
                    .on_press(Message::DesktopPicked(entry.path.clone()))
                    .padding([8, 12]);
                list = list.push(btn);
            }
        }
        container(column![
            text("Select Desktop File").size(20),
            text_input("Search...", &self.desktop_search).on_input(Message::DesktopSearch).padding(8),
            container(list).width(Length::Fixed(400.0)).height(Length::Fixed(400.0)),
            button(text("Cancel")).on_press(Message::HideDesktopPicker).padding([8, 24]),
        ].spacing(16).align_items(Alignment::Center)).padding(32).into()
    }

    fn view_picker(&self) -> Element<'_, Message> {
        let fp = match &self.file_picker { Some(f) => f, None => return column![].into() };
        let mut list = column![].spacing(4);
        let search_lc = fp.search.to_lowercase();
        for (name, is_dir) in &fp.entries {
            if search_lc.is_empty() || name.to_lowercase().contains(&search_lc) {
                let icon = if *is_dir { "📁" } else { "📄" };
                let btn = button(row![text(icon), text(name)].spacing(8))
                    .on_press(if *is_dir {
                        Message::PickerNavigate(fp.current_dir.join(name))
                    } else {
                        Message::PickerPicked(fp.current_dir.join(name).to_string_lossy().to_string())
                    })
                    .padding([8, 12]);
                list = list.push(btn);
            }
        }
        container(column![
            text(&fp.title).size(20),
            text_input("Search...", &fp.search).on_input(Message::PickerSearch).on_submit(Message::PickerSubmit).padding(8),
            container(list).width(Length::Fixed(500.0)).height(Length::Fixed(400.0)),
            button(text("Cancel")).on_press(Message::HidePicker).padding([8, 24]),
        ].spacing(16).align_items(Alignment::Center)).padding(32).into()
    }
}
