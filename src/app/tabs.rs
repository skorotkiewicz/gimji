use eframe::egui;

use crate::models::{Note, Tab, TabType};

use super::{ACTIVE_BG, ConfirmAction, GimjiApp, panel_frame};

impl GimjiApp {
    pub(super) fn render_note_header(&mut self, ui: &mut egui::Ui, note: &Note) {
        ui.horizontal(|ui| {
            if self.editing_note_title {
                let response = ui.add_sized(
                    [160.0, 28.0],
                    egui::TextEdit::singleline(&mut self.rename_note_title),
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
                if escape {
                    self.cancel_note_title_edit();
                } else if enter || ui.button("Save").clicked() {
                    self.rename_current_note();
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
                                        let mut save = enter || response.lost_focus();
                                        let mut cancel = escape;

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
                                    let job = tab_button_job(tab);
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

                            ui.menu_button("+", |ui| {
                                ui.set_min_width(120.0);
                                for tab_type in TabType::ALL {
                                    if ui.button(tab_type.label()).clicked() {
                                        self.add_tab(tab_type);
                                        ui.close();
                                    }
                                }
                            })
                            .response
                            .on_hover_text("Add tab");
                        });
                    });
            });
    }
}

pub(super) fn tab_button_job(tab: &Tab) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &tab.title,
        0.0,
        egui::TextFormat::simple(
            egui::FontId::proportional(14.0),
            tab_type_color(tab.tab_type),
        ),
    );
    job
}

pub(super) fn tab_type_color(tab_type: TabType) -> egui::Color32 {
    match tab_type {
        TabType::Markdown => egui::Color32::from_rgb(171, 209, 255),
        TabType::Kanban => egui::Color32::from_rgb(184, 221, 156),
        TabType::Todo => egui::Color32::from_rgb(245, 204, 112),
        TabType::Calendar => egui::Color32::from_rgb(216, 180, 254),
    }
}
