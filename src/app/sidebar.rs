use eframe::egui;

#[cfg(feature = "s3")]
use super::{ACCENT, S3ConnectionStatus};
use super::{GimjiApp, SIDEBAR_BG, SURFACE_HOVER, TEXT_MUTED};

const SIDEBAR_DEFAULT_WIDTH: f32 = 220.0;
const SIDEBAR_ROW_HEIGHT: f32 = 28.0;
const SIDEBAR_FIELD_HEIGHT: f32 = 30.0;
const SIDEBAR_RADIUS: u8 = 6;

impl GimjiApp {
    pub(super) fn render_sidebar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(SIDEBAR_DEFAULT_WIDTH)
            .size_range(200.0..=260.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::symmetric(14, 16)),
            )
            .show_inside(root_ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Gimji").size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new("Quit").small().frame(false).truncate())
                                .clicked()
                            {
                                ui.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });

                    ui.add_space(12.0);

                    section_label(ui, "Workspace");
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 6.0) / 2.0;
                        if ui
                            .add_sized(
                                [width, SIDEBAR_ROW_HEIGHT],
                                egui::Button::new("Open")
                                    .small()
                                    .corner_radius(SIDEBAR_RADIUS),
                            )
                            .clicked()
                        {
                            self.open_workspace_dialog();
                        }
                        if ui
                            .add_sized(
                                [width, SIDEBAR_ROW_HEIGHT],
                                egui::Button::new("New")
                                    .small()
                                    .corner_radius(SIDEBAR_RADIUS),
                            )
                            .clicked()
                        {
                            self.new_workspace_dialog();
                        }
                    });

                    #[cfg(feature = "s3")]
                    {
                        ui.add_space(12.0);
                        self.render_s3_section(ui);
                    }

                    if !self.recent.paths.is_empty() {
                        ui.add_space(12.0);
                        section_label(ui, "Recent");
                        for path in self.recent.paths.clone() {
                            let label = path.display().to_string();
                            let response = sidebar_row(ui, &shorten_path(&label), false)
                                .on_hover_text(format!("{label}\nRight-click for actions"));

                            response.context_menu(|ui| {
                                ui.set_min_width(120.0);
                                if ui.button("Delete").clicked() {
                                    self.remove_recent_workspace(&path);
                                    ui.close();
                                }
                            });

                            if response.clicked() {
                                self.open_workspace(path);
                            }
                        }
                    }

                    ui.add_space(12.0);

                    section_label(ui, "Notes");
                    ui.horizontal(|ui| {
                        let add_width = 32.0;
                        ui.add_sized(
                            [ui.available_width() - add_width - 6.0, SIDEBAR_FIELD_HEIGHT],
                            egui::TextEdit::singleline(&mut self.new_note_title)
                                .hint_text("New note")
                                .desired_width(f32::INFINITY)
                                .margin(egui::Vec2::new(6.0, 5.0)),
                        );
                        if ui
                            .add_sized(
                                [add_width, SIDEBAR_FIELD_HEIGHT],
                                egui::Button::new("+").small().corner_radius(SIDEBAR_RADIUS),
                            )
                            .clicked()
                        {
                            self.add_note();
                        }
                    });

                    ui.add_sized(
                        [ui.available_width(), SIDEBAR_FIELD_HEIGHT],
                        egui::TextEdit::singleline(&mut self.note_filter)
                            .hint_text("Filter notes")
                            .desired_width(f32::INFINITY)
                            .margin(egui::Vec2::new(6.0, 5.0)),
                    );

                    let notes: Vec<(String, String, bool)> = self
                        .workspace
                        .as_ref()
                        .map(|workspace| {
                            let selected = workspace.selected_note_id();
                            workspace
                                .config()
                                .notes
                                .iter()
                                .filter(|note| note_matches_filter(&note.title, &self.note_filter))
                                .map(|note| {
                                    (
                                        note.id.clone(),
                                        note.title.clone(),
                                        selected == Some(note.id.as_str()),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    egui::ScrollArea::vertical()
                        .id_salt("sidebar-notes")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (id, title, selected) in notes {
                                if sidebar_row(ui, &title, selected).clicked() {
                                    self.select_note(id);
                                }
                            }
                        });
                });
            });
    }

    #[cfg(feature = "s3")]
    pub(super) fn render_s3_section(&mut self, ui: &mut egui::Ui) {
        let label = if self.s3_settings_expanded {
            "S3 v"
        } else {
            "S3 >"
        };
        if sidebar_row(ui, label, self.s3_settings_expanded).clicked() {
            self.toggle_s3_settings();
        }

        if !self.s3_settings_expanded {
            return;
        }

        sidebar_text_field(ui, &mut self.s3_endpoint_url, "Endpoint URL", false);
        sidebar_text_field(ui, &mut self.s3_region, "Region", false);
        sidebar_text_field(ui, &mut self.s3_bucket, "Bucket", false);
        sidebar_text_field(ui, &mut self.s3_prefix, "Prefix", false);
        sidebar_text_field(ui, &mut self.s3_access_key_id, "Access key", false);
        sidebar_text_field(ui, &mut self.s3_secret_access_key, "Secret key", true);
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [74.0, SIDEBAR_ROW_HEIGHT],
                    egui::Button::new("Backup")
                        .small()
                        .corner_radius(SIDEBAR_RADIUS),
                )
                .clicked()
            {
                self.backup_workspace_to_s3();
            }
            if ui
                .add_sized(
                    [76.0, SIDEBAR_ROW_HEIGHT],
                    egui::Button::new("Restore")
                        .small()
                        .corner_radius(SIDEBAR_RADIUS),
                )
                .clicked()
            {
                self.request_s3_restore();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [64.0, SIDEBAR_ROW_HEIGHT],
                    egui::Button::new("Test")
                        .small()
                        .corner_radius(SIDEBAR_RADIUS),
                )
                .clicked()
            {
                self.test_s3_connection();
            }
        });
        ui.label(
            egui::RichText::new(self.s3_connection_status.label())
                .small()
                .color(s3_connection_status_color(&self.s3_connection_status)),
        );
    }
}

fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .small()
            .strong()
            .color(TEXT_MUTED),
    );
}

fn sidebar_row(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width(), SIDEBAR_ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });

    if ui.is_rect_visible(rect) {
        let fill = if selected {
            // ACTIVE_BG
            egui::Color32::TRANSPARENT
        } else if response.hovered() {
            SURFACE_HOVER
        } else {
            egui::Color32::TRANSPARENT
        };

        if fill != egui::Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, SIDEBAR_RADIUS, fill);
        }

        let text_rect = rect.shrink2(egui::vec2(8.0, 0.0));
        let text_color = if selected {
            egui::Color32::WHITE
        } else {
            ui.visuals().text_color()
        };
        ui.painter().with_clip_rect(text_rect).text(
            text_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.0),
            text_color,
        );
    }

    response
}

#[cfg(feature = "s3")]
fn sidebar_text_field(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    password: bool,
) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), SIDEBAR_FIELD_HEIGHT],
        egui::TextEdit::singleline(value)
            .password(password)
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .margin(egui::Vec2::new(6.0, 5.0)),
    )
}

fn shorten_path(path: &str) -> String {
    let components: Vec<&str> = path.split(std::path::MAIN_SEPARATOR).collect();
    if components.len() > 3 {
        format!(".../{}", components[components.len() - 1..].join("/"))
    } else {
        path.to_string()
    }
}

pub(super) fn note_matches_filter(title: &str, filter: &str) -> bool {
    let filter = filter.trim();
    filter.is_empty() || title.to_lowercase().contains(&filter.to_lowercase())
}

#[cfg(feature = "s3")]
fn s3_connection_status_color(status: &S3ConnectionStatus) -> egui::Color32 {
    match status {
        S3ConnectionStatus::Idle => TEXT_MUTED,
        S3ConnectionStatus::Testing => egui::Color32::from_rgb(102, 171, 238),
        S3ConnectionStatus::Connected => ACCENT,
        S3ConnectionStatus::Error(_) => egui::Color32::from_rgb(232, 116, 116),
    }
}
