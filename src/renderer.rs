use cairo::{Context, Format, ImageSurface};
use freedesktop_icons::lookup;
use std::path::PathBuf;
use std::collections::HashMap;
use std::fs;

use crate::app_launcher::Entry;
use crate::animations::DockAnimations;
use crate::config::DockConfig;

// ── Public constants ──────────────────────────────────────────────────────────

/// Pixels reserved above the panel for bounce animation overdraw.
pub const BOUNCE_MARGIN: u32 = 56;
/// Pixels reserved above BOUNCE_MARGIN for folder popup rendering.
pub const POPUP_RESERVE: u32 = 180;
/// Total pixels above the dock panel.
pub const OVERDRAW: u32 = POPUP_RESERVE + BOUNCE_MARGIN;

// ── DockRenderer ──────────────────────────────────────────────────────────────

pub struct DockRenderer {
    icon_cache: HashMap<String, Option<ImageSurface>>,
    icon_cache_dir: PathBuf,
    icon_map_path: PathBuf,
    icon_map: HashMap<String, String>,
}

impl DockRenderer {
    pub fn new() -> Self {
        let mut icon_cache_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        icon_cache_dir.push("rusty-dock");
        icon_cache_dir.push("icon_cache");
        let icon_map_path = icon_cache_dir.join("icon_map");
        let _ = fs::create_dir_all(&icon_cache_dir);
        let icon_map = Self::load_icon_map(&icon_map_path);
        Self { icon_cache: HashMap::new(), icon_cache_dir, icon_map_path, icon_map }
    }

    fn load_icon_map(path: &PathBuf) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(c) = fs::read_to_string(path) {
            for line in c.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }
        map
    }

    fn save_icon_map(&self) {
        let content: String = self.icon_map.iter()
            .map(|(k, v)| format!("{}={}\n", k, v))
            .collect();
        let _ = fs::write(&self.icon_map_path, content);
    }

    fn process_and_cache_icon(&mut self, source_path: &str, entry_name: &str) -> Option<PathBuf> {
        if let Some(cached_name) = self.icon_map.get(source_path) {
            let p = self.icon_cache_dir.join(cached_name);
            if p.exists() { return Some(p); }
        }
        let source = PathBuf::from(source_path);
        if !source.exists() { return None; }

        // Handle SVG icons with resvg
        if source_path.ends_with(".svg") {
            return self.process_svg_with_resvg(source_path, entry_name);
        }

        // Handle raster icons with image crate
        let safe_name: String = entry_name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let output_name = format!("{}.png", safe_name);
        let output_path = self.icon_cache_dir.join(&output_name);

        self.process_with_image_crate(source_path, &output_path)
    }

    fn process_svg_with_resvg(&mut self, source_path: &str, entry_name: &str) -> Option<PathBuf> {
        if let Some(cached_name) = self.icon_map.get(source_path) {
            let p = self.icon_cache_dir.join(cached_name);
            if p.exists() { return Some(p); }
        }

        let source = PathBuf::from(source_path);
        if !source.exists() { return None; }

        let svg_data = fs::read_to_string(&source).ok()?;
        let safe_name: String = entry_name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let output_name = format!("{}.png", safe_name);
        let output_path = self.icon_cache_dir.join(&output_name);

        // Parse and render SVG using resvg
        let tree = resvg::usvg::Tree::from_str(&svg_data, &resvg::usvg::Options::default()).ok()?;
        let pixmap_size = tree.size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 128)?;

        let transform = resvg::tiny_skia::Transform::from_scale(
            128.0 / pixmap_size.width(),
            128.0 / pixmap_size.height(),
        );

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Save as PNG
        let png_data = pixmap.encode_png().ok()?;
        fs::write(&output_path, png_data).ok()?;

        self.icon_map.insert(source_path.to_string(), output_name);
        self.save_icon_map();
        Some(output_path)
    }

    fn process_with_image_crate(&mut self, source_path: &str, output_path: &PathBuf) -> Option<PathBuf> {
        if let Ok(img) = image::open(source_path) {
            let img = img.resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
            if img.save(output_path).is_ok() {
                self.icon_map.insert(source_path.to_string(),
                                     output_path.file_name()?.to_str()?.to_string());
                self.save_icon_map();
                return Some(output_path.clone());
            }
        }
        None
    }

    // ── Main render ───────────────────────────────────────────────────────────

    /// Render the full dock surface.
    pub fn render(
        &mut self,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        apps: &[Entry],
        hovered: Option<usize>,
        dragging: Option<usize>,
        drag_offset: f64,
        visibility: f64,
        animations: &mut DockAnimations,
        config: &DockConfig,
        open_folder: Option<usize>,
        hovered_folder_app: Option<usize>,
    ) {
        let stride = width as i32 * 4;
        let surface = unsafe {
            ImageSurface::create_for_data_unsafe(
                buffer.as_mut_ptr(), Format::ARgb32,
                width as i32, height as i32, stride,
            ).expect("Failed to create Cairo surface")
        };
        let ctx = Context::new(&surface).expect("Failed to create Cairo context");

        // Clear entire surface
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().unwrap();
        ctx.set_operator(cairo::Operator::Over);

        if visibility < 0.01 {
            drop(ctx); surface.flush(); return;
        }

        let panel_y      = OVERDRAW as f64;
        let panel_h      = config.dock_height as f64;
        let icon_size    = config.icon_size as i32;
        let icon_padding = config.icon_padding as i32;
        let n            = apps.len() as i32;

        // Panel background
        {
            ctx.save().unwrap();
            self.draw_rounded_rect(&ctx, 0.0, panel_y, width as f64, panel_h, config.border_radius);
            ctx.set_source_rgba(0.1, 0.1, 0.1, config.background_opacity);
            ctx.fill_preserve().unwrap();
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.1);
            ctx.set_line_width(1.0);
            ctx.stroke().unwrap();
            ctx.restore().unwrap();
        }

        animations.ensure_capacity(apps.len());

        if n == 0 { drop(ctx); surface.flush(); return; }

        let total_w = n * (icon_size + icon_padding * 2);
        let start_x  = ((width as i32 - total_w) / 2) as f64;
        let base_icon_y = panel_y + (panel_h - icon_size as f64) / 2.0;

        // Draw each dock icon
        for (i, app) in apps.iter().enumerate() {
            let mut x = start_x + (i as i32 * (icon_size + icon_padding * 2)) as f64;
            let is_hovered  = hovered == Some(i);
            let is_dragging = dragging == Some(i);

            let scale  = animations.get_icon_scale(i);
            let bounce = animations.get_bounce_offset(i);

            let icon_y = base_icon_y + bounce;

            if is_dragging { x += drag_offset; }

            // Shift neighbours while dragging
            if let Some(drag_idx) = dragging {
                if i != drag_idx {
                    let drag_x = start_x + (drag_idx as i32 * (icon_size + icon_padding * 2)) as f64 + drag_offset;
                    let icon_cx = x + icon_size as f64 / 2.0 + icon_padding as f64;
                    let drag_cx = drag_x + icon_size as f64 / 2.0 + icon_padding as f64;
                    let shift = (icon_size + icon_padding * 2) as f64;
                    if drag_idx < i && drag_cx > icon_cx { x -= shift; }
                    else if drag_idx > i && drag_cx < icon_cx { x += shift; }
                }
            }

            let icon_cx = x + icon_padding as f64 + icon_size as f64 / 2.0;

            ctx.save().unwrap();
            if (scale - 1.0).abs() > 0.001 {
                ctx.translate(icon_cx, icon_y + icon_size as f64 / 2.0);
                ctx.scale(scale, scale);
                ctx.translate(-icon_cx, -(icon_y + icon_size as f64 / 2.0));
            }

            if app.is_spacer {
                self.draw_spacer(&ctx, x, base_icon_y, icon_size, icon_padding);
            } else if app.is_folder {
                self.draw_folder_icon(&ctx, app, x, icon_y, is_hovered, is_dragging,
                                      icon_size, icon_padding, config);
            } else {
                self.draw_app_icon(&ctx, app, x, icon_y, is_hovered, is_dragging,
                                   icon_size, icon_padding, config);
            }

            // Running indicator dot
            if config.show_active_indicators && !is_dragging {
                if app.is_active {
                    ctx.save().unwrap();
                    let dot_x = icon_cx;
                    let dot_y = panel_y + panel_h - 5.0;
                    ctx.arc(dot_x, dot_y, 3.0, 0.0, std::f64::consts::TAU);
                    ctx.set_source_rgba(0.4, 0.8, 1.0, 0.9);
                    ctx.fill().unwrap();
                    ctx.restore().unwrap();
                } else if app.is_folder && app.folder_entries.iter().any(|fe| fe.is_active) {
                    ctx.save().unwrap();
                    let dot_x = icon_cx;
                    let dot_y = panel_y + panel_h - 5.0;
                    ctx.arc(dot_x, dot_y, 3.0, 0.0, std::f64::consts::TAU);
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                    ctx.fill().unwrap();
                    ctx.restore().unwrap();
                }
            }

            // Tooltip
            if is_hovered && config.show_app_names && !is_dragging {
                ctx.save().unwrap();
                ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                ctx.set_font_size(12.0);
                let ext = ctx.text_extents(&app.name).unwrap();
                let tw = ext.width() + 12.0;
                let th = ext.height() + 8.0;
                let tx = icon_cx - tw / 2.0;
                let ty = icon_y - th - 8.0;
                self.draw_rounded_rect(&ctx, tx, ty, tw, th, 4.0);
                ctx.set_source_rgba(0.1, 0.1, 0.1, 0.9);
                ctx.fill().unwrap();
                ctx.set_source_rgb(1.0, 1.0, 1.0);
                ctx.move_to(tx + 6.0, ty + th - 4.0);
                ctx.show_text(&app.name).unwrap();
                ctx.restore().unwrap();
            }

            ctx.restore().unwrap();
        }

        // Folder popup
        if let Some(folder_idx) = open_folder {
            if let Some(folder) = apps.get(folder_idx) {
                if !folder.folder_entries.is_empty() {
                    let folder_x = start_x + (folder_idx as i32 * (icon_size + icon_padding * 2)) as f64;
                    let folder_cx = folder_x + icon_padding as f64 + icon_size as f64 / 2.0;
                    self.draw_folder_popup(
                        &ctx, folder, folder_cx, width, config,
                        hovered_folder_app,
                    );
                }
            }
        }

        drop(ctx);
        surface.flush();
    }

    // ── Folder popup ─────────────────────────────────────────────────────────

    fn draw_folder_popup(
        &mut self,
        ctx: &Context,
        folder: &Entry,
        anchor_cx: f64,
        surface_width: u32,
        config: &DockConfig,
        hovered_app: Option<usize>,
    ) {
        let cols      = config.folder_popup_columns.max(1) as usize;
        let icon_sz   = config.folder_popup_icon_size as f64;
        let cell_pad  = 10.0_f64;
        let cell_w    = icon_sz + cell_pad * 2.0;
        let cell_h    = icon_sz + cell_pad * 2.0;
        let n         = folder.folder_entries.len();
        let rows      = (n + cols - 1) / cols;
        let pop_w     = cols as f64 * cell_w + 16.0;
        let pop_h     = rows as f64 * cell_h + 16.0 + 20.0;

        let pop_x = (anchor_cx - pop_w / 2.0)
            .max(4.0)
            .min(surface_width as f64 - pop_w - 4.0);
        let pop_bottom = POPUP_RESERVE as f64;
        let pop_y = pop_bottom - pop_h - 4.0;

        ctx.save().unwrap();
        self.draw_rounded_rect(ctx, pop_x, pop_y, pop_w, pop_h, 12.0);
        ctx.set_source_rgba(0.12, 0.12, 0.15, 0.95);
        ctx.fill_preserve().unwrap();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        ctx.set_line_width(1.0);
        ctx.stroke().unwrap();

        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(12.0);
        ctx.set_source_rgb(0.9, 0.9, 0.9);
        let ext = ctx.text_extents(&folder.name).unwrap();
        ctx.move_to(pop_x + (pop_w - ext.width()) / 2.0, pop_y + 16.0);
        ctx.show_text(&folder.name).unwrap();

        let grid_top = pop_y + 24.0;
        for (idx, fe) in folder.folder_entries.iter().enumerate() {
            let col = (idx % cols) as f64;
            let row = (idx / cols) as f64;
            let cx = pop_x + 8.0 + col * cell_w + cell_pad;
            let cy = grid_top + row * cell_h + cell_pad;

            let is_hov = hovered_app == Some(idx);

            if is_hov {
                ctx.save().unwrap();
                self.draw_rounded_rect(ctx, cx - cell_pad + 2.0, cy - cell_pad + 2.0,
                                       cell_w - 4.0, cell_h - 4.0, 8.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
                ctx.fill().unwrap();
                ctx.restore().unwrap();
            }

            let tmp_entry = crate::app_launcher::Entry {
                name: fe.name.clone(),
                exec: fe.exec.clone(),
                args: fe.args.clone(),
                icon: fe.icon.clone(),
                desktop_file: None,
                is_active: fe.is_active,
                is_special: false, is_spacer: false, is_folder: false,
                show_miniatures: false,
                folder_entries: vec![],
            };
            let icon_surf = self.load_icon(&tmp_entry, icon_sz as i32);
            if let Some(surf) = icon_surf {
                ctx.save().unwrap();
                self.draw_rounded_rect(ctx, cx, cy, icon_sz, icon_sz, 6.0);
                ctx.clip();
                let ox = (icon_sz - surf.width() as f64) / 2.0;
                let oy = (icon_sz - surf.height() as f64) / 2.0;
                ctx.set_source_surface(&surf, cx + ox, cy + oy).unwrap();
                ctx.paint().unwrap();
                ctx.restore().unwrap();
            } else {
                ctx.save().unwrap();
                self.draw_rounded_rect(ctx, cx, cy, icon_sz, icon_sz, 6.0);
                ctx.set_source_rgb(0.4, 0.4, 0.5);
                ctx.fill().unwrap();
                if let Some(ch) = fe.name.chars().next() {
                    ctx.set_source_rgb(1.0,1.0,1.0);
                    ctx.set_font_size(20.0);
                    let t = ch.to_uppercase().to_string();
                    let e = ctx.text_extents(&t).unwrap();
                    ctx.move_to(cx + (icon_sz - e.width())/2.0, cy + (icon_sz + e.height())/2.0);
                    ctx.show_text(&t).unwrap();
                }
                ctx.restore().unwrap();
            }

            ctx.save().unwrap();
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            ctx.set_font_size(10.0);
            ctx.set_source_rgba(0.9, 0.9, 0.9, 0.9);
            let label = if fe.name.len() > 10 { &fe.name[..10] } else { &fe.name };
            let le = ctx.text_extents(label).unwrap();
            ctx.move_to(cx + (icon_sz - le.width()) / 2.0, cy + icon_sz + 12.0);
            ctx.show_text(label).unwrap();

            if fe.is_active && config.show_active_indicators {
                ctx.arc(cx + icon_sz / 2.0, cy + icon_sz + 18.0, 2.5, 0.0, std::f64::consts::TAU);
                ctx.set_source_rgba(0.4, 0.8, 1.0, 0.9);
                ctx.fill().unwrap();
            }
            ctx.restore().unwrap();
        }

        ctx.restore().unwrap();
    }

    // ── Icon drawing helpers ─────────────────────────────────────────────────

    fn draw_spacer(&self, ctx: &Context, x: f64, y: f64, icon_size: i32, icon_padding: i32) {
        ctx.save().unwrap();
        let cx = x + icon_padding as f64 + icon_size as f64 / 2.0;
        let half = icon_size as f64 * 0.3;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.25);
        ctx.set_line_width(1.5);
        ctx.move_to(cx, y + icon_size as f64 / 2.0 - half);
        ctx.line_to(cx, y + icon_size as f64 / 2.0 + half);
        ctx.stroke().unwrap();
        ctx.restore().unwrap();
    }

    fn draw_folder_icon(
        &mut self, ctx: &Context, app: &Entry,
        x: f64, y: f64, hovered: bool, dragging: bool,
        icon_size: i32, icon_padding: i32, config: &DockConfig,
    ) {
        ctx.save().unwrap();
        let ix = x + icon_padding as f64;
        let sz = icon_size as f64;

        if hovered && !dragging && config.enable_animations {
            self.draw_rounded_rect(ctx, ix - 4.0, y - 4.0, sz + 8.0, sz + 8.0, 10.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
            ctx.fill().unwrap();
        }

        if !app.show_miniatures {
            let surf = self.load_icon(app, icon_size);
            if let Some(s) = surf {
                self.draw_rounded_rect(ctx, ix, y, sz, sz, 6.0);
                ctx.clip();
                let ox = (sz - s.width() as f64) / 2.0;
                let oy = (sz - s.height() as f64) / 2.0;
                ctx.set_source_surface(&s, ix + ox, y + oy).unwrap();
                ctx.paint().unwrap();
            } else {
                self.draw_rounded_rect(ctx, ix, y, sz, sz, 6.0);
                ctx.set_source_rgba(0.3, 0.3, 0.45, 0.9);
                ctx.fill().unwrap();
                if let Some(ch) = app.name.chars().next() {
                    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    ctx.set_font_size(sz * 0.5);
                    ctx.set_source_rgb(1.0, 1.0, 1.0);
                    let t = ch.to_uppercase().to_string();
                    let ext = ctx.text_extents(&t).unwrap();
                    ctx.move_to(ix + (sz - ext.width()) / 2.0, y + (sz + ext.height()) / 2.0);
                    ctx.show_text(&t).unwrap();
                }
            }
        } else {
            let preview_count = app.folder_entries.len().min(4);
            if preview_count == 0 {
                self.draw_rounded_rect(ctx, ix, y, sz, sz, 8.0);
                ctx.set_source_rgba(0.3, 0.3, 0.45, 0.9);
                ctx.fill().unwrap();
            } else {
                let bg_r = 8.0_f64;
                self.draw_rounded_rect(ctx, ix, y, sz, sz, bg_r);
                ctx.set_source_rgba(0.22, 0.22, 0.32, 0.92);
                ctx.fill().unwrap();

                let padding = 4.0_f64;
                let inner_sz = (sz - padding * 3.0) / 2.0;
                let positions = [
                    (ix + padding,             y + padding),
                    (ix + padding * 2.0 + inner_sz, y + padding),
                    (ix + padding,             y + padding * 2.0 + inner_sz),
                    (ix + padding * 2.0 + inner_sz, y + padding * 2.0 + inner_sz),
                ];
                for (idx, fe) in app.folder_entries.iter().take(4).enumerate() {
                    let (px, py) = positions[idx];
                    let tmp = crate::app_launcher::Entry {
                        name: fe.name.clone(), exec: fe.exec.clone(),
                        args: fe.args.clone(),
                        icon: fe.icon.clone(), desktop_file: None,
                        is_active: false, is_special: false, is_spacer: false,
                        is_folder: false, show_miniatures: false,
                        folder_entries: vec![],
                    };
                    let surf = self.load_icon(&tmp, inner_sz as i32);
                    ctx.save().unwrap();
                    self.draw_rounded_rect(ctx, px, py, inner_sz, inner_sz, 3.0);
                    if let Some(s) = surf {
                        ctx.clip();
                        let ox = (inner_sz - s.width() as f64) / 2.0;
                        let oy = (inner_sz - s.height() as f64) / 2.0;
                        ctx.set_source_surface(&s, px + ox, py + oy).unwrap();
                        ctx.paint().unwrap();
                    } else {
                        ctx.set_source_rgb(0.5, 0.5, 0.6);
                        ctx.fill().unwrap();
                    }
                    ctx.restore().unwrap();
                }
            }
        }


        ctx.restore().unwrap();
    }

    fn draw_app_icon(
        &mut self, ctx: &Context, app: &Entry,
        x: f64, y: f64, hovered: bool, dragging: bool,
        icon_size: i32, icon_padding: i32, config: &DockConfig,
    ) {
        let ix = x + icon_padding as f64;
        let sz = icon_size as f64;

        if hovered && !dragging && config.enable_animations {
            ctx.save().unwrap();
            self.draw_rounded_rect(ctx, ix - 4.0, y - 4.0, sz + 8.0, sz + 8.0, 10.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
            ctx.fill().unwrap();
            ctx.restore().unwrap();
        }

        if dragging {
            ctx.save().unwrap();
            self.draw_rounded_rect(ctx, ix + 2.0, y + 4.0, sz, sz, 8.0);
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.45);
            ctx.fill().unwrap();
            ctx.restore().unwrap();
        }

        if app.is_special {
            ctx.save().unwrap();
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(22.0);
            ctx.set_source_rgb(0.9, 0.9, 0.9);
            let text = "•••";
            let ext = ctx.text_extents(text).unwrap();
            ctx.move_to(ix + (sz - ext.width()) / 2.0, y + (sz + ext.height()) / 2.0);
            ctx.show_text(text).unwrap();
            ctx.restore().unwrap();
            return;
        }

        let surf = self.load_icon(app, icon_size);
        if let Some(s) = surf {
            ctx.save().unwrap();
            self.draw_rounded_rect(ctx, ix, y, sz, sz, 6.0);
            ctx.clip();
            let ox = (sz - s.width() as f64) / 2.0;
            let oy = (sz - s.height() as f64) / 2.0;
            ctx.set_source_surface(&s, ix + ox, y + oy).unwrap();
            ctx.paint().unwrap();
            ctx.restore().unwrap();
        } else {
            ctx.save().unwrap();
            self.draw_rounded_rect(ctx, ix, y, sz, sz, 6.0);
            ctx.set_source_rgb(0.35, 0.35, 0.45);
            ctx.fill().unwrap();
            if let Some(ch) = app.name.chars().next() {
                ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                ctx.set_font_size(28.0);
                ctx.set_source_rgb(1.0, 1.0, 1.0);
                let t = ch.to_uppercase().to_string();
                let ext = ctx.text_extents(&t).unwrap();
                ctx.move_to(ix + (sz - ext.width()) / 2.0, y + (sz + ext.height()) / 2.0);
                ctx.show_text(&t).unwrap();
            }
            ctx.restore().unwrap();
        }
    }

    // ── Icon loading ─────────────────────────────────────────────────

    fn load_icon(&mut self, app: &Entry, icon_size: i32) -> Option<ImageSurface> {
        let icon_key = format!("{}_{}_{}_{}",
            app.name,
            app.icon.as_deref().unwrap_or(""),
            app.desktop_file.as_ref().map(|p| p.to_string_lossy()).unwrap_or_default(),
            icon_size
        );

        if let Some(cached) = self.icon_cache.get(&icon_key) {
            return cached.clone();
        }

        let icon_path = self.find_icon_path(app);
        let result = icon_path.and_then(|path| {
            let path_str = path.to_string_lossy();
            let entry_name = path.file_stem()?.to_string_lossy();

            if let Some(cached_name) = self.icon_map.get(path_str.as_ref()) {
                let p = self.icon_cache_dir.join(cached_name);
                if p.exists() {
                    return self.load_processed_icon(&p, icon_size);
                }
            }

            if path_str.ends_with(".svg") {
                self.process_svg_with_resvg(path_str.as_ref(), entry_name.as_ref())
                    .and_then(|p| self.load_processed_icon(&p, icon_size))
            } else {
                self.process_with_image_crate(path_str.as_ref(), &PathBuf::from(path_str.as_ref()))
                    .and_then(|p| self.load_processed_icon(&p, icon_size))
            }
        });

        self.icon_cache.insert(icon_key, result.clone());
        result
    }

    fn find_icon_path(&self, app: &Entry) -> Option<PathBuf> {
        if let Some(ref icon) = app.icon {
            if std::path::Path::new(icon).exists() {
                return Some(PathBuf::from(icon));
            }
            if let Some(icon_path) = lookup(icon).find() {
                return Some(icon_path);
            }
        }

        if let Some(ref desktop_file) = app.desktop_file {
            if let Ok(bytes) = fs::read_to_string(desktop_file) {
                if let Ok(entry) = freedesktop_desktop_entry::DesktopEntry::from_str(desktop_file, &bytes, None::<&[&str]>) {
                    if let Some(icon_name) = entry.icon() {
                        if let Some(icon_path) = lookup(icon_name).find() {
                            return Some(icon_path);
                        }
                    }
                }
            }
        }

        None
    }

    fn load_processed_icon(&self, path: &PathBuf, icon_size: i32) -> Option<ImageSurface> {
        let img = image::open(path).ok()?;
        let img = img.resize_exact(icon_size as u32, icon_size as u32, image::imageops::FilterType::Lanczos3);
        let rgba = img.to_rgba8();

        let mut surface = ImageSurface::create(Format::ARgb32, icon_size, icon_size).ok()?;
        let stride = surface.stride() as usize;
        {
            let mut data = surface.data().ok()?;
            for y in 0..icon_size as usize {
                for x in 0..icon_size as usize {
                    let pixel = rgba.get_pixel(x as u32, y as u32);
                    let offset = y * stride + x * 4;
                    data[offset] = pixel[2];
                    data[offset + 1] = pixel[1];
                    data[offset + 2] = pixel[0];
                    data[offset + 3] = pixel[3];
                }
            }
        }

        Some(surface)
    }

    // ── Helper ────────────────────────────────────────────────────────────────

    fn draw_rounded_rect(&self, ctx: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
        let min = w.min(h);
        let r = r.min(min / 2.0);
        ctx.new_path();
        ctx.move_to(x + r, y);
        ctx.line_to(x + w - r, y);
        ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(x + w, y + h - r);
        ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(x + r, y + h);
        ctx.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(x, y + r);
        ctx.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.close_path();
    }

    pub fn folder_app_at(
        &self,
        folder_idx: usize,
        x: f64,
        y: f64,
        apps: &[Entry],
        surface_width: u32,
        config: &DockConfig,
    ) -> Option<usize> {
        if let Some(folder) = apps.get(folder_idx) {
            if let Some((px, py, pw, ph)) = self.popup_rect(folder_idx, apps, surface_width, config) {
                if x >= px && x <= px + pw && y >= py && y <= py + ph {
                    let cols = config.folder_popup_columns.max(1) as usize;
                    let icon_sz = config.folder_popup_icon_size as f64;
                    let cell_pad = 10.0_f64;
                    let cell_w = icon_sz + cell_pad * 2.0;
                    let cell_h = icon_sz + cell_pad * 2.0;

                    let grid_left = px + 8.0 + cell_pad;
                    let grid_top = py + 24.0 + cell_pad;

                    let col = ((x - grid_left) / cell_w).floor() as usize;
                    let row = ((y - grid_top) / cell_h).floor() as usize;
                    let idx = row * cols + col;

                    if idx < folder.folder_entries.len() {
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    pub fn popup_rect(
        &self,
        folder_idx: usize,
        apps: &[Entry],
        surface_width: u32,
        config: &DockConfig,
    ) -> Option<(f64, f64, f64, f64)> {
        if let Some(folder) = apps.get(folder_idx) {
            if folder.folder_entries.is_empty() {
                return None;
            }

            let cols = config.folder_popup_columns.max(1) as usize;
            let icon_sz = config.folder_popup_icon_size as f64;
            let cell_pad = 10.0_f64;
            let cell_w = icon_sz + cell_pad * 2.0;
            let cell_h = icon_sz + cell_pad * 2.0;
            let n = folder.folder_entries.len();
            let rows = (n + cols - 1) / cols;
            let pop_w = cols as f64 * cell_w + 16.0;
            let pop_h = rows as f64 * cell_h + 16.0 + 20.0;

            let icon_size = config.icon_size as i32;
            let icon_padding = config.icon_padding as i32;
            let n_dock = apps.len() as i32;
            let total_w = n_dock * (icon_size + icon_padding * 2);
            let start_x = (surface_width as i32 - total_w) / 2;
            let folder_x = start_x + (folder_idx as i32 * (icon_size + icon_padding * 2));
            let folder_cx = folder_x as f64 + icon_padding as f64 + icon_size as f64 / 2.0;

            let pop_x = (folder_cx - pop_w / 2.0)
                .max(4.0)
                .min(surface_width as f64 - pop_w - 4.0);
            let pop_bottom = POPUP_RESERVE as f64;
            let pop_y = pop_bottom - pop_h - 4.0;

            Some((pop_x, pop_y, pop_w, pop_h))
        } else {
            None
        }
    }
}
