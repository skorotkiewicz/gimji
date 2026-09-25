use chrono::Local;
use eframe::egui;

use crate::models::{CalendarData, CalendarEvent, KanbanBoard, KanbanCard, TodoItem, TodoList};

use super::{STROKE, SURFACE_BG, SURFACE_LOW, TEXT_MUTED, panel_frame};

pub(super) const KANBAN_COLUMN_WIDTH: f32 = 280.0;
pub(super) const KANBAN_CARD_TEXT_WIDTH: f32 = 250.0;
pub(super) const KANBAN_CARD_TEXT_HEIGHT: f32 = 76.0;
const MARKDOWN_MIN_VISIBLE_ROWS: usize = 24;
const TODO_DUPLICATE_STROKE: egui::Color32 = egui::Color32::from_rgb(232, 116, 116);

pub(super) fn render_markdown(ui: &mut egui::Ui, markdown: &mut String) -> bool {
    panel_frame(SURFACE_LOW)
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

    panel_frame(SURFACE_LOW).show(ui, |ui| {
        egui::ScrollArea::new(kanban_scroll_axes()).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for column_index in 0..board.columns.len() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(KANBAN_COLUMN_WIDTH, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(SURFACE_BG)
                                .inner_margin(egui::Margin::same(10))
                                .corner_radius(6)
                                .stroke(egui::Stroke::new(1.0_f32, STROKE))
                                .show(ui, |ui| {
                                    ui.set_width(KANBAN_COLUMN_WIDTH);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(&board.columns[column_index].title)
                                                .size(16.0)
                                                .strong(),
                                        );
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
                                            .fill(SURFACE_LOW)
                                            .inner_margin(egui::Margin::same(8))
                                            .corner_radius(4)
                                            .stroke(egui::Stroke::new(1.0_f32, STROKE))
                                            .show(ui, |ui| {
                                                ui.set_width(KANBAN_CARD_TEXT_WIDTH);
                                                let card = &mut board.columns[column_index].cards
                                                    [card_index];
                                                let mut expanded = !card.details_hidden;
                                                ui.horizontal(|ui| {
                                                    let (_, response) = ui.allocate_exact_size(
                                                        egui::vec2(14.0, 14.0),
                                                        egui::Sense::click(),
                                                    );
                                                    let response =
                                                        response.on_hover_text(if expanded {
                                                            "Hide details"
                                                        } else {
                                                            "Show details"
                                                        });
                                                    egui::collapsing_header::paint_default_icon(
                                                        ui,
                                                        if expanded { 1.0 } else { 0.0 },
                                                        &response,
                                                    );
                                                    if response.clicked() {
                                                        expanded = !expanded;
                                                        card.details_hidden = !expanded;
                                                        card.touch();
                                                        dirty = true;
                                                    }
                                                    if ui
                                                        .add_sized(
                                                            egui::vec2(ui.available_width(), 28.0),
                                                            egui::TextEdit::singleline(
                                                                &mut card.name,
                                                            )
                                                            .hint_text("Name"),
                                                        )
                                                        .changed()
                                                    {
                                                        card.touch();
                                                        dirty = true;
                                                    }
                                                });
                                                if expanded {
                                                    if ui
                                                        .add_sized(
                                                            egui::vec2(
                                                                KANBAN_CARD_TEXT_WIDTH,
                                                                KANBAN_CARD_TEXT_HEIGHT,
                                                            ),
                                                            egui::TextEdit::multiline(
                                                                &mut card.description,
                                                            )
                                                            .hint_text("Description")
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
                                                            action =
                                                                Some(KanbanAction::MoveColumn {
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
                                                            action =
                                                                Some(KanbanAction::MoveColumn {
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
                                                            action =
                                                                Some(KanbanAction::DeleteCard {
                                                                    column_index,
                                                                    card_index,
                                                                });
                                                        }
                                                    });
                                                }
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

pub(super) fn render_todo(ui: &mut egui::Ui, todo: &mut TodoList, tab_id: &str) -> bool {
    let mut dirty = false;
    let mut delete_index = None;
    let mut focus_new_item = false;
    let mut all_tags = todo
        .items
        .iter()
        .flat_map(|item| item.tags.iter().cloned())
        .collect::<Vec<_>>();
    all_tags.sort_by_key(|tag| tag.to_lowercase());
    all_tags.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    let filter_id = ui.make_persistent_id(("todo-tag-filter", tab_id));
    let mut tag_filter = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(filter_id).unwrap_or_default());
    if !tag_filter.is_empty()
        && !all_tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(&tag_filter))
    {
        tag_filter.clear();
    }

    panel_frame(SURFACE_LOW).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Tasks").size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new("+ Todo").small().corner_radius(6))
                    .clicked()
                {
                    todo.items.push(new_todo_item());
                    tag_filter.clear();
                    focus_new_item = true;
                    dirty = true;
                }
            });
        });

        if !all_tags.is_empty() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter").small().color(TEXT_MUTED));
                egui::ComboBox::from_id_salt(filter_id)
                    .selected_text(if tag_filter.is_empty() {
                        "All tags"
                    } else {
                        tag_filter.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut tag_filter, String::new(), "All tags");
                        for tag in &all_tags {
                            ui.selectable_value(
                                &mut tag_filter,
                                tag.clone(),
                                egui::RichText::new(tag).color(tag_color(tag)),
                            );
                        }
                    });
            });
        }

        let has_visible_items = todo
            .items
            .iter()
            .any(|item| todo_matches_tag(item, &tag_filter));
        if !has_visible_items {
            ui.add_space(12.0);
            let message = if todo.items.is_empty() {
                "No tasks"
            } else {
                "No tasks with this tag"
            };
            ui.label(egui::RichText::new(message).color(TEXT_MUTED));
        }

        let focus_index = focus_new_item.then(|| todo.items.len().saturating_sub(1));
        let duplicates = duplicate_todo_items(&todo.items);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, item) in todo.items.iter_mut().enumerate() {
                if !todo_matches_tag(item, &tag_filter) {
                    continue;
                }

                egui::Frame::new()
                    .fill(SURFACE_BG)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .corner_radius(4)
                    .stroke(egui::Stroke::new(1.0_f32, STROKE))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut item.done, "").changed() {
                                item.touch();
                                dirty = true;
                            }
                            let response = ui
                                .scope(|ui| {
                                    if duplicates[index] {
                                        let stroke =
                                            egui::Stroke::new(1.0_f32, TODO_DUPLICATE_STROKE);
                                        ui.visuals_mut().widgets.inactive.bg_stroke = stroke;
                                        ui.visuals_mut().widgets.hovered.bg_stroke = stroke;
                                        ui.visuals_mut().widgets.active.bg_stroke = stroke;
                                        ui.visuals_mut().selection.stroke = stroke;
                                    }
                                    ui.add_sized(
                                        [ui.available_width() - 50.0, 28.0],
                                        egui::TextEdit::singleline(&mut item.text)
                                            .hint_text("Todo"),
                                    )
                                })
                                .inner;
                            if focus_index == Some(index) {
                                response.request_focus();
                            }
                            if response.changed() {
                                item.touch();
                                dirty = true;
                                ui.ctx().request_repaint();
                            }
                            if ui
                                .small_button("Del")
                                .on_hover_text("Delete todo")
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });

                        let draft_id = ui.make_persistent_id(("todo-tag-draft", item.id.as_str()));
                        let mut tag_draft = ui
                            .ctx()
                            .data_mut(|data| data.get_temp::<String>(draft_id).unwrap_or_default());
                        let mut remove_tag_index = None;
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Tags").small().color(TEXT_MUTED));
                            for (tag_index, tag) in item.tags.iter().enumerate() {
                                let color = tag_color(tag);
                                let chip = egui::Button::new(
                                    egui::RichText::new(format!("#{tag} x"))
                                        .small()
                                        .color(color),
                                )
                                .small()
                                .fill(color.gamma_multiply(0.15))
                                .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.4)))
                                .corner_radius(6);
                                if ui.add(chip).on_hover_text("Remove tag").clicked() {
                                    remove_tag_index = Some(tag_index);
                                }
                            }

                            let response = ui.add_sized(
                                [120.0, 24.0],
                                egui::TextEdit::singleline(&mut tag_draft).hint_text("Add tag"),
                            );
                            let enter_pressed = response.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter));
                            let add_clicked =
                                ui.small_button("Add").on_hover_text("Add tag").clicked();
                            if (enter_pressed || add_clicked) && add_todo_tag(item, &tag_draft) {
                                tag_draft.clear();
                                dirty = true;
                            }
                        });

                        if let Some(tag_index) = remove_tag_index {
                            item.tags.remove(tag_index);
                            item.touch();
                            dirty = true;
                        }
                        ui.ctx()
                            .data_mut(|data| data.insert_temp(draft_id, tag_draft));
                    });
                ui.add_space(6.0);
            }
        });
    });

    ui.ctx()
        .data_mut(|data| data.insert_temp(filter_id, tag_filter));

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

    panel_frame(SURFACE_LOW).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Calendar").size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new("+ Event").small().corner_radius(6))
                    .clicked()
                {
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
                    .fill(SURFACE_BG)
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(4)
                    .stroke(egui::Stroke::new(1.0_f32, STROKE))
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

fn duplicate_todo_items(items: &[TodoItem]) -> Vec<bool> {
    // ponytail: Todo lists are small; replace this O(n²) scan with counts if that changes.
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            !item.text.is_empty()
                && items
                    .iter()
                    .enumerate()
                    .any(|(other_index, other)| index != other_index && item.text == other.text)
        })
        .collect()
}

fn add_todo_tag(item: &mut TodoItem, tag: &str) -> bool {
    let tag = tag.trim();
    if tag.is_empty()
        || item
            .tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(tag))
    {
        return false;
    }

    item.tags.push(tag.to_owned());
    item.touch();
    true
}

fn todo_matches_tag(item: &TodoItem, tag: &str) -> bool {
    tag.is_empty()
        || item
            .tags
            .iter()
            .any(|item_tag| item_tag.eq_ignore_ascii_case(tag))
}

fn tag_color(tag: &str) -> egui::Color32 {
    const TAG_COLORS: [egui::Color32; 13] = [
        egui::Color32::from_rgb(239, 68, 68),  // red
        egui::Color32::from_rgb(249, 115, 22), // orange
        egui::Color32::from_rgb(245, 158, 11), // amber
        egui::Color32::from_rgb(234, 179, 8),  // yellow
        egui::Color32::from_rgb(132, 204, 22), // lime
        egui::Color32::from_rgb(34, 197, 94),  // green
        egui::Color32::from_rgb(16, 185, 129), // emerald
        egui::Color32::from_rgb(20, 184, 166), // teal
        egui::Color32::from_rgb(6, 182, 212),  // cyan
        egui::Color32::from_rgb(14, 165, 233), // sky
        egui::Color32::from_rgb(59, 130, 246), // blue
        egui::Color32::from_rgb(236, 72, 153), // pink
        egui::Color32::from_rgb(244, 63, 94),  // rose
    ];

    // Color hash: hash = char + (hash << 5) - hash, case-insensitive.
    let hash = tag
        .chars()
        .map(|ch| ch.to_ascii_lowercase() as i32)
        .fold(0_i32, |hash, ch| hash.wrapping_mul(31).wrapping_add(ch));
    TAG_COLORS[hash.unsigned_abs() as usize % TAG_COLORS.len()]
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

#[cfg(test)]
mod tests {
    use super::{TodoItem, add_todo_tag, duplicate_todo_items, todo_matches_tag};

    #[test]
    fn todo_duplicates_require_exact_non_empty_text() {
        let items = ["Ship", "Ship", "ship", "Ship ", "", ""].map(TodoItem::new);

        assert_eq!(
            duplicate_todo_items(&items),
            [true, true, false, false, false, false]
        );
    }

    #[test]
    fn todo_tags_are_trimmed_unique_and_filter_case_insensitively() {
        let mut item = TodoItem::new("Ship");

        assert!(add_todo_tag(&mut item, " work "));
        assert!(!add_todo_tag(&mut item, "WORK"));
        assert!(!add_todo_tag(&mut item, " "));
        assert_eq!(item.tags, ["work"]);
        assert!(todo_matches_tag(&item, "Work"));
        assert!(todo_matches_tag(&item, ""));
        assert!(!todo_matches_tag(&item, "personal"));
    }

    #[test]
    fn tag_color_is_stable_across_tag_case() {
        assert_eq!(super::tag_color("Work"), super::tag_color("WORK"));
    }
}
