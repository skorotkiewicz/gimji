use eframe::egui;

use crate::models::{Note, Tab, TabType};

use super::{ConfirmAction, GimjiApp, SURFACE_HOVER};

const TAB_CHIP_HEIGHT: f32 = 30.0;
const TAB_CHIP_MIN_WIDTH: f32 = 82.0;
const TAB_CHIP_MAX_WIDTH: f32 = 190.0;
const TAB_ADD_WIDTH: f32 = 32.0;
const TAB_CHIP_RADIUS: u8 = 6;

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
        egui::ScrollArea::horizontal()
            .id_salt("tab-row")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.set_min_height(TAB_CHIP_HEIGHT);
                    let selected_tab = self
                        .workspace
                        .as_ref()
                        .and_then(|workspace| workspace.selected_tab_id())
                        .map(str::to_owned);
                    for tab in &note.tabs {
                        let selected = selected_tab.as_deref() == Some(tab.id.as_str());

                        if selected
                            && self.renaming_tab
                            && self.rename_tab_id.as_deref() == Some(tab.id.as_str())
                        {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                let response = ui.add_sized(
                                    [150.0, TAB_CHIP_HEIGHT],
                                    egui::TextEdit::singleline(&mut self.rename_tab_title)
                                        .hint_text("Tab name")
                                        .margin(egui::Vec2::new(6.0, 4.0)),
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

                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    if ui
                                        .add(
                                            egui::Button::new("Save")
                                                .small()
                                                .corner_radius(TAB_CHIP_RADIUS),
                                        )
                                        .clicked()
                                    {
                                        save = true;
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new("Cancel")
                                                .small()
                                                .corner_radius(TAB_CHIP_RADIUS),
                                        )
                                        .clicked()
                                    {
                                        cancel = true;
                                    }
                                });

                                if cancel {
                                    self.cancel_tab_title_edit();
                                } else if save {
                                    self.save_tab_title_edit();
                                }
                            });
                        } else {
                            let mut response = tab_chip(ui, tab, selected);
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
                                    self.request_delete(ConfirmAction::DeleteTab(tab.id.clone()));
                                    ui.close();
                                }
                            });

                            if response.clicked() {
                                self.select_tab(tab.id.clone());
                            }
                        }
                    }

                    let (response, _) = egui::containers::menu::MenuButton::from_button(
                        egui::Button::new("+")
                            .small()
                            .min_size(egui::vec2(TAB_ADD_WIDTH, TAB_CHIP_HEIGHT))
                            .corner_radius(TAB_CHIP_RADIUS),
                    )
                    .ui(ui, |ui| {
                        ui.set_min_width(120.0);
                        for tab_type in TabType::ALL {
                            if ui.button(tab_type.label()).clicked() {
                                self.add_tab(tab_type);
                                ui.close();
                            }
                        }
                    });
                    response.on_hover_text("Add tab");
                });
            });
    }
}

fn tab_chip(ui: &mut egui::Ui, tab: &Tab, selected: bool) -> egui::Response {
    let job = tab_button_job(tab);
    let desired_size = tab_chip_size(&job.text);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            &job.text,
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
            ui.painter().rect_filled(rect, TAB_CHIP_RADIUS, fill);
        }

        let accent = tab_type_color(tab.tab_type);
        let dot_center = egui::pos2(rect.left() + 12.0, rect.center().y);
        ui.painter().circle_filled(dot_center, 3.0, accent);

        if selected {
            let accent_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 8.0, rect.bottom() - 3.0),
                egui::pos2(rect.right() - 8.0, rect.bottom() - 1.0),
            );
            ui.painter().rect_filled(accent_rect, 1.0, accent);
        }

        let text_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 22.0, rect.top()),
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
            &job.text,
            egui::FontId::proportional(14.0),
            text_color,
        );
    }

    response
}

pub(super) fn tab_chip_size(title: &str) -> egui::Vec2 {
    let width =
        (title.chars().count() as f32 * 7.0 + 34.0).clamp(TAB_CHIP_MIN_WIDTH, TAB_CHIP_MAX_WIDTH);
    egui::vec2(width, TAB_CHIP_HEIGHT)
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
