use chrono::Local;
use eframe::egui;

use crate::models::{CalendarData, CalendarEvent, KanbanBoard, KanbanCard, TodoItem, TodoList};

use super::{SURFACE_BG, TEXT_MUTED, panel_frame};

pub(super) const KANBAN_COLUMN_WIDTH: f32 = 280.0;
pub(super) const KANBAN_CARD_TEXT_WIDTH: f32 = 250.0;
pub(super) const KANBAN_CARD_TEXT_HEIGHT: f32 = 76.0;
const MARKDOWN_MIN_VISIBLE_ROWS: usize = 24;

pub(super) fn render_markdown(ui: &mut egui::Ui, markdown: &mut String) -> bool {
    panel_frame(SURFACE_BG)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("markdown-editor-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let desired_rows = markdown_editor_desired_rows(markdown);
                    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                    let editor_height = row_height * desired_rows as f32;
                    ui.add_sized(
                        egui::vec2(ui.available_width(), editor_height),
                        egui::TextEdit::multiline(markdown)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Write markdown...")
                            .desired_width(f32::INFINITY)
                            .desired_rows(desired_rows),
                    )
                    .changed()
                })
                .inner
        })
        .inner
}

pub(super) fn markdown_editor_desired_rows(markdown: &str) -> usize {
    markdown.lines().count().max(MARKDOWN_MIN_VISIBLE_ROWS)
}

pub(super) fn render_kanban(ui: &mut egui::Ui, board: &mut KanbanBoard) -> bool {
    let mut dirty = false;
    let mut action = None;

    panel_frame(SURFACE_BG).show(ui, |ui| {
        egui::ScrollArea::new(kanban_scroll_axes()).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for column_index in 0..board.columns.len() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(KANBAN_COLUMN_WIDTH, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(25, 28, 32))
                                .inner_margin(egui::Margin::same(10))
                                .corner_radius(6)
                                .show(ui, |ui| {
                                    ui.set_width(KANBAN_COLUMN_WIDTH);
                                    ui.horizontal(|ui| {
                                        ui.heading(&board.columns[column_index].title);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(egui::Button::new("+ Card").small())
                                                    .clicked()
                                                {
                                                    action =
                                                        Some(KanbanAction::AddCard(column_index));
                                                }
                                            },
                                        );
                                    });

                                    let card_count = board.columns[column_index].cards.len();
                                    if card_count == 0 {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new("No cards")
                                                .small()
                                                .color(TEXT_MUTED),
                                        );
                                    }

                                    for card_index in 0..card_count {
                                        ui.add_space(8.0);
                                        egui::Frame::new()
                                            .fill(SURFACE_BG)
                                            .inner_margin(egui::Margin::same(8))
                                            .corner_radius(4)
                                            .show(ui, |ui| {
                                                ui.set_width(KANBAN_CARD_TEXT_WIDTH);
                                                let card = &mut board.columns[column_index].cards
                                                    [card_index];
                                                if ui
                                                    .add_sized(
                                                        egui::vec2(
                                                            KANBAN_CARD_TEXT_WIDTH,
                                                            KANBAN_CARD_TEXT_HEIGHT,
                                                        ),
                                                        egui::TextEdit::multiline(&mut card.text)
                                                            .desired_rows(3),
                                                    )
                                                    .changed()
                                                {
                                                    card.touch();
                                                    dirty = true;
                                                }
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .small_button("<")
                                                        .on_hover_text("Move left")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveColumn {
                                                            column_index,
                                                            card_index,
                                                            delta: -1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button(">")
                                                        .on_hover_text("Move right")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveColumn {
                                                            column_index,
                                                            card_index,
                                                            delta: 1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Up")
                                                        .on_hover_text("Move up")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveRow {
                                                            column_index,
                                                            card_index,
                                                            delta: -1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Dn")
                                                        .on_hover_text("Move down")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::MoveRow {
                                                            column_index,
                                                            card_index,
                                                            delta: 1,
                                                        });
                                                    }
                                                    if ui
                                                        .small_button("Del")
                                                        .on_hover_text("Delete card")
                                                        .clicked()
                                                    {
                                                        action = Some(KanbanAction::DeleteCard {
                                                            column_index,
                                                            card_index,
                                                        });
                                                    }
                                                });
                                            });
                                    }
                                });
                        },
                    );
                }
            });
        });
    });

    if let Some(action) = action {
        apply_kanban_action(board, action);
        dirty = true;
    }

    dirty
}

pub(super) fn render_todo(ui: &mut egui::Ui, todo: &mut TodoList) -> bool {
    let mut dirty = false;
    let mut delete_index = None;
    let mut focus_new_item = false;

    panel_frame(SURFACE_BG).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Tasks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Todo").clicked() {
                    todo.items.push(new_todo_item());
                    focus_new_item = true;
                    dirty = true;
                }
            });
        });

        if todo.items.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No tasks").color(TEXT_MUTED));
        }

        let focus_index = focus_new_item.then(|| todo.items.len().saturating_sub(1));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, item) in todo.items.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 28, 32))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut item.done, "").changed() {
                                item.touch();
                                dirty = true;
                            }
                            let response = ui.add_sized(
                                [ui.available_width() - 50.0, 28.0],
                                egui::TextEdit::singleline(&mut item.text).hint_text("Todo"),
                            );
                            if focus_index == Some(index) {
                                response.request_focus();
                            }
                            if response.changed() {
                                item.touch();
                                dirty = true;
                            }
                            if ui
                                .small_button("Del")
                                .on_hover_text("Delete todo")
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });
    });

    if let Some(index) = delete_index {
        todo.items.remove(index);
        dirty = true;
    }

    dirty
}

pub(super) fn render_calendar(ui: &mut egui::Ui, calendar: &mut CalendarData) -> bool {
    let mut dirty = false;
    let mut delete_index = None;
    let mut focus_new_event = false;

    calendar.events.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then(left.title.cmp(&right.title))
    });

    panel_frame(SURFACE_BG).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Calendar");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Event").clicked() {
                    calendar.events.push(new_calendar_event(
                        Local::now().format("%Y-%m-%d").to_string(),
                    ));
                    focus_new_event = true;
                    dirty = true;
                }
            });
        });

        if calendar.events.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No events").color(TEXT_MUTED));
        }

        let focus_index = focus_new_event.then(|| calendar.events.len().saturating_sub(1));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, event) in calendar.events.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(25, 28, 32))
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Date").small().color(TEXT_MUTED));
                            if ui
                                .add_sized(
                                    [112.0, 28.0],
                                    egui::TextEdit::singleline(&mut event.date),
                                )
                                .changed()
                            {
                                event.touch();
                                dirty = true;
                            }
                            ui.label(egui::RichText::new("Title").small().color(TEXT_MUTED));
                            let response = ui.add_sized(
                                [ui.available_width() - 50.0, 28.0],
                                egui::TextEdit::singleline(&mut event.title)
                                    .hint_text("Event title"),
                            );
                            if focus_index == Some(index) {
                                response.request_focus();
                            }
                            if response.changed() {
                                event.touch();
                                dirty = true;
                            }
                            if ui
                                .small_button("Del")
                                .on_hover_text("Delete event")
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut event.description)
                                    .hint_text("Description")
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        {
                            event.touch();
                            dirty = true;
                        }
                    });
                ui.add_space(6.0);
            }
        });
    });

    if let Some(index) = delete_index {
        calendar.events.remove(index);
        dirty = true;
    }

    dirty
}

pub(super) fn new_todo_item() -> TodoItem {
    TodoItem::new("")
}

pub(super) fn new_calendar_event(date: String) -> CalendarEvent {
    CalendarEvent::new(date, "", "")
}

#[cfg(test)]
pub(super) fn kanban_column_header_action_area_size(width: f32) -> egui::Vec2 {
    egui::vec2(width, super::NOTE_HEADER_ACTION_HEIGHT)
}

pub(super) fn kanban_scroll_axes() -> [bool; 2] {
    [true, true]
}

#[cfg(test)]
pub(super) fn kanban_column_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_COLUMN_WIDTH, 0.0)
}

#[cfg(test)]
pub(super) fn kanban_card_text_area_size() -> egui::Vec2 {
    egui::vec2(KANBAN_CARD_TEXT_WIDTH, KANBAN_CARD_TEXT_HEIGHT)
}

#[derive(Debug, Clone, Copy)]
enum KanbanAction {
    AddCard(usize),
    DeleteCard {
        column_index: usize,
        card_index: usize,
    },
    MoveColumn {
        column_index: usize,
        card_index: usize,
        delta: isize,
    },
    MoveRow {
        column_index: usize,
        card_index: usize,
        delta: isize,
    },
}

fn apply_kanban_action(board: &mut KanbanBoard, action: KanbanAction) {
    match action {
        KanbanAction::AddCard(column_index) => {
            if let Some(column) = board.columns.get_mut(column_index) {
                column.cards.push(KanbanCard::new("New card"));
            }
        }
        KanbanAction::DeleteCard {
            column_index,
            card_index,
        } => {
            if let Some(column) = board.columns.get_mut(column_index)
                && card_index < column.cards.len()
            {
                column.cards.remove(card_index);
            }
        }
        KanbanAction::MoveColumn {
            column_index,
            card_index,
            delta,
        } => {
            let destination = column_index as isize + delta;
            if destination < 0 || destination >= board.columns.len() as isize {
                return;
            }
            if card_index >= board.columns[column_index].cards.len() {
                return;
            }
            let card = board.columns[column_index].cards.remove(card_index);
            board.columns[destination as usize].cards.push(card);
        }
        KanbanAction::MoveRow {
            column_index,
            card_index,
            delta,
        } => {
            let Some(column) = board.columns.get_mut(column_index) else {
                return;
            };
            let destination = card_index as isize + delta;
            if destination < 0 || destination >= column.cards.len() as isize {
                return;
            }
            let dest = destination as usize;
            column.cards.swap(card_index, dest);
        }
    }
}
