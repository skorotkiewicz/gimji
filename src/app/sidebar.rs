use eframe::egui;

#[cfg(feature = "s3")]
use super::S3ConnectionStatus;
use super::{ACCENT, ConfirmAction, GimjiApp, SIDEBAR_BG, STROKE, SURFACE_HOVER, TEXT_MUTED};

const SIDEBAR_DEFAULT_WIDTH: f32 = 220.0;
const SIDEBAR_ROW_HEIGHT: f32 = 28.0;
const SIDEBAR_FIELD_HEIGHT: f32 = 30.0;
const SIDEBAR_RADIUS: u8 = 6;

fn close_icon_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
    let response = response.on_hover_text("Quit");
    let visuals = ui.style().interact(&response);
    let rect = rect.shrink(2.0).expand(visuals.expansion);

    ui.painter()
        .line_segment([rect.left_top(), rect.right_bottom()], visuals.fg_stroke);
    ui.painter()
        .line_segment([rect.right_top(), rect.left_bottom()], visuals.fg_stroke);

    response
}

impl GimjiApp {
    pub(super) fn render_sidebar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(SIDEBAR_DEFAULT_WIDTH)
            .size_range(200.0..=260.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::symmetric(14, 16))
                    .stroke(egui::Stroke::new(1.0_f32, STROKE)),
            )
            .show_inside(root_ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Gimji").size(21.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if close_icon_button(ui).clicked() {
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
                                if selected && self.editing_note_title {
                                    self.render_sidebar_note_rename_editor(ui);
                                    continue;
                                }

                                let response = sidebar_row(ui, &title, selected)
                                    .on_hover_text("Right-click for actions");

                                response.context_menu(|ui| {
                                    ui.set_min_width(120.0);
                                    if ui.button("Rename").clicked() {
                                        if !selected {
                                            self.select_note(id.clone());
                                        } else {
                                            self.refresh_rename_buffers();
                                        }
                                        self.editing_note_title = true;
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui.button("Delete").clicked() {
                                        self.request_delete(ConfirmAction::DeleteNote(id.clone()));
                                        ui.close();
                                    }
                                });

                                if response.clicked() {
                                    self.select_note(id);
                                }
                            }
                        });
                });
            });
    }

    fn render_sidebar_note_rename_editor(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            let response = ui.add_sized(
                [ui.available_width(), SIDEBAR_FIELD_HEIGHT],
                egui::TextEdit::singleline(&mut self.rename_note_title)
                    .hint_text("Note name")
                    .desired_width(f32::INFINITY)
                    .margin(egui::Vec2::new(6.0, 5.0)),
            );
            let (enter, escape) = if response.has_focus() {
                ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                })
            } else {
                (false, false)
            };
            let mut save = enter;
            let mut cancel = escape;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let width = (ui.available_width() - 6.0) / 2.0;
                if ui
                    .add_sized(
                        [width, 18.0],
                        egui::Button::new("Save")
                            .small()
                            .corner_radius(SIDEBAR_RADIUS),
                    )
                    .clicked()
                {
                    save = true;
                }
                if ui
                    .add_sized(
                        [width, 18.0],
                        egui::Button::new("Cancel")
                            .small()
                            .corner_radius(SIDEBAR_RADIUS),
                    )
                    .clicked()
                {
                    cancel = true;
                }
            });

            if cancel {
                self.cancel_note_title_edit();
            } else if save {
                self.rename_current_note();
            }
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
                    [74.0, 24.0],
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
                    [76.0, 24.0],
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
                    [64.0, 18.0],
                    egui::Button::new("Save")
                        .small()
                        .corner_radius(SIDEBAR_RADIUS),
                )
                .on_hover_text("Save S3 settings in this workspace")
                .clicked()
            {
                self.save_s3_connection_settings();
            }
            if ui
                .add_sized(
                    [64.0, 18.0],
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

        let marker_color = if selected {
            ACCENT
        } else if response.hovered() {
            TEXT_MUTED
        } else {
            egui::Color32::from_rgb(82, 91, 96)
        };
        ui.painter().circle_filled(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            2.5,
            marker_color,
        );
        // if selected {
        //     let accent_rect = egui::Rect::from_min_size(
        //         egui::pos2(rect.left(), rect.top() + 7.0),
        //         egui::vec2(2.0, rect.height() - 14.0),
        //     );
        //     ui.painter().rect_filled(accent_rect, 1.0, ACCENT);
        // }

        let text_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 18.0, rect.top()),
            egui::pos2(rect.right() - 8.0, rect.bottom()),
        );
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
