use eframe::egui;

use crate::models::{Note, TabType};

use super::{ACTIVE_BG, ConfirmAction, GimjiApp, TEXT_MUTED, panel_frame};

impl GimjiApp {
    pub(super) fn render_note_header(&mut self, ui: &mut egui::Ui, note: &Note) {
        ui.horizontal(|ui| {
            if self.editing_note_title {
                let response = ui.add_sized(
                    [ui.available_width() - 80.0, 28.0],
                    egui::TextEdit::singleline(&mut self.rename_note_title),
                );
                let save_shortcut =
                    response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                let cancel_shortcut =
                    response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape));
                if cancel_shortcut {
                    self.cancel_note_title_edit();
                } else if save_shortcut || ui.button("Save").clicked() {
                    self.save_note_title_edit();
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_note_title_edit();
                }
            } else {
                ui.heading(&note.title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Delete Note")
                        .on_hover_text("Remove note metadata only")
                        .clicked()
                    {
                        self.request_delete(ConfirmAction::DeleteNote(note.id.clone()));
                    }
                    if ui.button("Rename").clicked() {
                        self.editing_note_title = true;
                        self.refresh_rename_buffers();
                    }
                });
            }
        });
    }

    pub(super) fn render_tab_row(&mut self, ui: &mut egui::Ui, note: &Note) {
        panel_frame(egui::Color32::from_rgb(25, 27, 30))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("tab-row")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let selected_tab = self
                                .workspace
                                .as_ref()
                                .and_then(|workspace| workspace.selected_tab_id())
                                .map(str::to_owned);
                            for tab in &note.tabs {
                                let selected = selected_tab.as_deref() == Some(tab.id.as_str());
                                let fill = if selected {
                                    ACTIVE_BG
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                if selected
                                    && self.renaming_tab
                                    && self.rename_tab_id.as_deref() == Some(tab.id.as_str())
                                {
                                    ui.horizontal(|ui| {
                                        let response = ui.add_sized(
                                            [140.0, 28.0],
                                            egui::TextEdit::singleline(&mut self.rename_tab_title)
                                                .hint_text("Tab name")
                                                .margin(egui::Vec2::new(4.0, 2.0)),
                                        );

                                        let mut save = false;
                                        let mut cancel = false;

                                        if response.has_focus()
                                            && ui
                                                .input(|input| input.key_pressed(egui::Key::Escape))
                                        {
                                            cancel = true;
                                        }
                                        if response.has_focus()
                                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                                        {
                                            save = true;
                                        }
                                        if response.lost_focus() {
                                            save = true;
                                        }
                                        if ui.small_button("Save").clicked() {
                                            save = true;
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            cancel = true;
                                        }

                                        if cancel {
                                            self.cancel_tab_title_edit();
                                        } else if save {
                                            self.save_tab_title_edit();
                                        }
                                    });
                                } else {
                                    let type_text = tab.tab_type.as_str();
                                    let mut job = egui::text::LayoutJob::default();
                                    job.append(
                                        &tab.title,
                                        0.0,
                                        egui::TextFormat::simple(
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::WHITE,
                                        ),
                                    );
                                    job.append(
                                        format!(" ({type_text})").as_str(),
                                        0.0,
                                        egui::TextFormat::simple(
                                            egui::FontId::proportional(12.0),
                                            TEXT_MUTED,
                                        ),
                                    );
                                    let mut response = ui.add(
                                        egui::Button::new(job)
                                            .selected(selected)
                                            .fill(fill)
                                            .sense(egui::Sense::click()),
                                    );
                                    response = response.on_hover_text(format!(
                                        "Type: {}\nRight-click for actions",
                                        tab.tab_type.label()
                                    ));

                                    response.context_menu(|ui| {
                                        ui.set_min_width(120.0);
                                        if ui.button("Rename").clicked() {
                                            if !selected {
                                                self.select_tab(tab.id.clone());
                                            }
                                            self.renaming_tab = true;
                                            self.rename_tab_id = Some(tab.id.clone());
                                            self.refresh_rename_buffers();
                                            ui.close();
                                        }
                                        ui.separator();
                                        if ui.button("Delete").clicked() {
                                            self.request_delete(ConfirmAction::DeleteTab(
                                                tab.id.clone(),
                                            ));
                                            ui.close();
                                        }
                                    });

                                    if response.clicked() {
                                        self.select_tab(tab.id.clone());
                                    }
                                }
                            }
                        });
                    });
            });
    }

    pub(super) fn render_tab_toolbar(&mut self, ui: &mut egui::Ui) {
        panel_frame(egui::Color32::from_rgb(25, 27, 30))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("New:").small().color(TEXT_MUTED));

                    egui::ComboBox::from_id_salt("new-tab-type")
                        .width(100.0)
                        .selected_text(self.new_tab_type.label())
                        .show_ui(ui, |ui| {
                            for tab_type in TabType::ALL {
                                ui.selectable_value(
                                    &mut self.new_tab_type,
                                    tab_type,
                                    tab_type.label(),
                                );
                            }
                        });

                    ui.add_sized(
                        [160.0, 24.0],
                        egui::TextEdit::singleline(&mut self.new_tab_title)
                            .hint_text("Title")
                            .margin(egui::Vec2::new(4.0, 2.0)),
                    );

                    if ui
                        .add_sized([40.0, 24.0], egui::Button::new("Add").small())
                        .clicked()
                    {
                        self.add_tab();
                    }
                });
            });
    }
}

#[cfg(test)]
pub(super) fn tab_action_section_titles() -> [&'static str; 1] {
    ["Create Tab"]
}
