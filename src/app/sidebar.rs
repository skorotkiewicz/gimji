use eframe::egui;

#[cfg(feature = "s3")]
use super::{ACCENT, S3ConnectionStatus};
use super::{ACTIVE_BG, GimjiApp, SIDEBAR_BG, TEXT_MUTED};

impl GimjiApp {
    pub(super) fn render_sidebar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(200.0)
            .size_range(180.0..=200.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::symmetric(14, 14)),
            )
            .show_inside(root_ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Gimji");
                    ui.label(egui::RichText::new("Workspace").small().color(TEXT_MUTED));
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 6.0) / 2.0;
                        if ui
                            .add_sized([width, 28.0], egui::Button::new("Open"))
                            .clicked()
                        {
                            self.open_workspace_dialog();
                        }
                        if ui
                            .add_sized([width, 28.0], egui::Button::new("New"))
                            .clicked()
                        {
                            self.new_workspace_dialog();
                        }
                    });

                    #[cfg(feature = "s3")]
                    {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        self.render_s3_section(ui);
                    }

                    if !self.recent.paths.is_empty() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        section_label(ui, "Recent");
                        for path in self.recent.paths.clone() {
                            let label = path.display().to_string();
                            let response = ui
                                .add_sized(
                                    [ui.available_width(), 24.0],
                                    egui::Button::new(shorten_path(&label))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .small(),
                                )
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
                    ui.separator();
                    ui.add_space(8.0);

                    section_label(ui, "Notes");
                    ui.horizontal(|ui| {
                        let add_width = 34.0;
                        ui.add_sized(
                            [ui.available_width() - add_width - 6.0, 26.0],
                            egui::TextEdit::singleline(&mut self.new_note_title)
                                .hint_text("New note title"),
                        );
                        if ui
                            .add_sized([add_width, 26.0], egui::Button::new("+").small())
                            .clicked()
                        {
                            self.add_note();
                        }
                    });

                    ui.add(
                        egui::TextEdit::singleline(&mut self.note_filter)
                            .hint_text("Filter")
                            .desired_width(f32::INFINITY)
                            .margin(egui::Vec2::new(4.0, 4.0)),
                    );

                    ui.add_space(6.0);

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
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (id, title, selected) in notes {
                                let bg = if selected {
                                    ACTIVE_BG
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 30.0],
                                        egui::Button::new(egui::RichText::new(title).size(14.0))
                                            .fill(bg)
                                            .frame(false)
                                            .selected(selected),
                                    )
                                    .clicked()
                                {
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
            "v S3"
        } else {
            "> S3"
        };
        if ui
            .add_sized(
                [ui.available_width(), 26.0],
                egui::Button::new(label)
                    .small()
                    .fill(egui::Color32::TRANSPARENT),
            )
            .clicked()
        {
            self.toggle_s3_settings();
        }

        if !self.s3_settings_expanded {
            return;
        }

        ui.add(
            egui::TextEdit::singleline(&mut self.s3_endpoint_url)
                .hint_text("Endpoint URL")
                .desired_width(f32::INFINITY)
                .margin(egui::Vec2::new(4.0, 4.0)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.s3_region)
                .hint_text("Region")
                .desired_width(f32::INFINITY)
                .margin(egui::Vec2::new(4.0, 4.0)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.s3_bucket)
                .hint_text("Bucket")
                .desired_width(f32::INFINITY)
                .margin(egui::Vec2::new(4.0, 4.0)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.s3_access_key_id)
                .hint_text("Access key")
                .desired_width(f32::INFINITY)
                .margin(egui::Vec2::new(4.0, 4.0)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.s3_secret_access_key)
                .password(true)
                .hint_text("Secret key")
                .desired_width(f32::INFINITY)
                .margin(egui::Vec2::new(4.0, 4.0)),
        );
        ui.horizontal(|ui| {
            if ui
                .add_sized([74.0, 26.0], egui::Button::new("Backup").small())
                .clicked()
            {
                self.backup_workspace_to_s3();
            }
            if ui
                .add_sized([76.0, 26.0], egui::Button::new("Restore").small())
                .clicked()
            {
                self.request_s3_restore();
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_sized([64.0, 26.0], egui::Button::new("Test").small())
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
        egui::RichText::new(label)
            .small()
            .strong()
            .color(TEXT_MUTED),
    );
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
