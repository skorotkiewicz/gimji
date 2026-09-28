use std::sync::OnceLock;

use chrono::Local;
use comrak::{
    Arena, Options,
    nodes::{ListType, Node, NodeValue},
};
use eframe::egui;
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::models::{CalendarData, CalendarEvent, KanbanBoard, KanbanCard, TodoItem, TodoList};

use super::{STROKE, SURFACE_BG, SURFACE_LOW, TEXT_MUTED, panel_frame};

pub(super) const KANBAN_COLUMN_WIDTH: f32 = 280.0;
pub(super) const KANBAN_CARD_TEXT_WIDTH: f32 = 250.0;
pub(super) const KANBAN_CARD_TEXT_HEIGHT: f32 = 76.0;
const MARKDOWN_MIN_VISIBLE_ROWS: usize = 24;
const TODO_DUPLICATE_STROKE: egui::Color32 = egui::Color32::from_rgb(232, 116, 116);

pub(super) fn render_markdown(ui: &mut egui::Ui, markdown: &mut String) -> bool {
    let id = ui.make_persistent_id("markdown-preview");
    let mut preview = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if ui.selectable_label(preview, "Preview").clicked() {
        preview = !preview;
        ui.data_mut(|data| data.insert_temp(id, preview));
    }

    panel_frame(SURFACE_LOW)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("markdown-editor-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if preview {
                        render_markdown_preview(ui, markdown);
                        false
                    } else {
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
                    }
                })
                .inner
        })
        .inner
}

fn render_markdown_preview(ui: &mut egui::Ui, markdown: &str) {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    let document = comrak::parse_document(&arena, markdown, &options);
    for child in document.children() {
        render_markdown_block(ui, child);
    }
}

fn render_markdown_block(ui: &mut egui::Ui, node: Node<'_>) {
    match &node.data().value {
        NodeValue::Heading(heading) => {
            let size = (28.0 - heading.level as f32 * 2.0).max(16.0);
            ui.add_space(6.0);
            ui.label(inline_job(node, size, ui.visuals().text_color()));
        }
        NodeValue::Paragraph | NodeValue::TableCell => {
            ui.label(inline_job(node, 14.0, ui.visuals().text_color()));
        }
        NodeValue::CodeBlock(block) => {
            let language = block.info.split_whitespace().next().unwrap_or("");
            egui::Frame::new()
                .fill(SURFACE_BG)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.label(code_job(&block.literal, language));
                });
        }
        NodeValue::List(list) => {
            for (index, child) in node.children().enumerate() {
                let marker = match list.list_type {
                    ListType::Ordered => format!("{}. ", list.start + index),
                    ListType::Bullet => "• ".to_owned(),
                };
                ui.horizontal_top(|ui| {
                    ui.label(marker);
                    ui.vertical(|ui| {
                        for item in child.children() {
                            render_markdown_block(ui, item);
                        }
                    });
                });
            }
        }
        NodeValue::BlockQuote => {
            egui::Frame::new()
                .fill(SURFACE_BG)
                .inner_margin(8)
                .show(ui, |ui| {
                    for child in node.children() {
                        render_markdown_block(ui, child);
                    }
                });
        }
        NodeValue::Table(_) => {
            egui::Grid::new(node as *const _)
                .striped(true)
                .show(ui, |ui| {
                    for row in node.children() {
                        for cell in row.children() {
                            render_markdown_block(ui, cell);
                        }
                        ui.end_row();
                    }
                });
        }
        NodeValue::ThematicBreak => {
            ui.separator();
        }
        NodeValue::HtmlBlock(html) => {
            ui.label(&html.literal);
        }
        _ => {
            for child in node.children() {
                render_markdown_block(ui, child);
            }
        }
    }
}

fn inline_job(node: Node<'_>, size: f32, color: egui::Color32) -> egui::text::LayoutJob {
    fn append(node: Node<'_>, job: &mut egui::text::LayoutJob, format: egui::TextFormat) {
        let mut format = format;
        match &node.data().value {
            NodeValue::Text(text) => job.append(text, 0.0, format.clone()),
            NodeValue::Code(code) => {
                format.font_id = egui::FontId::monospace(format.font_id.size);
                job.append(&code.literal, 0.0, format.clone());
            }
            NodeValue::SoftBreak | NodeValue::LineBreak => job.append("\n", 0.0, format.clone()),
            NodeValue::Strong => format.font_id.size += 1.0,
            NodeValue::Emph => format.italics = true,
            NodeValue::Strikethrough => {
                format.strikethrough = egui::Stroke::new(1.0_f32, format.color)
            }
            NodeValue::Link(_) => format.underline = egui::Stroke::new(1.0_f32, format.color),
            NodeValue::TaskItem(task) => job.append(
                if task.symbol.is_some() {
                    "☑ "
                } else {
                    "☐ "
                },
                0.0,
                format.clone(),
            ),
            NodeValue::HtmlInline(text) => job.append(text, 0.0, format.clone()),
            _ => {}
        }
        for child in node.children() {
            append(child, job, format.clone());
        }
    }
    let mut job = egui::text::LayoutJob::default();
    append(
        node,
        &mut job,
        egui::TextFormat {
            font_id: egui::FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    job
}

fn code_job(code: &str, language: &str) -> egui::text::LayoutJob {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    let syntax = syntaxes
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &themes.themes["base16-eighties.dark"]);
    let mut job = egui::text::LayoutJob::default();
    for line in LinesWithEndings::from(code) {
        match highlighter.highlight_line(line, syntaxes) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    job.append(
                        text,
                        0.0,
                        egui::TextFormat {
                            font_id: egui::FontId::monospace(14.0),
                            color: egui::Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ),
                            ..Default::default()
                        },
                    );
                }
            }
            Err(_) => job.append(
                line,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::monospace(14.0),
                    ..Default::default()
                },
            ),
        }
    }
    job
}

#[cfg(test)]
#[test]
fn preview_parses_markdown_and_highlights_fenced_code() {
    let arena = Arena::new();
    let document = comrak::parse_document(
        &arena,
        "# Title\n\n**bold** and `code`\n",
        &Options::default(),
    );
    let heading = document.first_child().unwrap();
    assert_eq!(
        inline_job(heading, 24.0, egui::Color32::WHITE).text,
        "Title"
    );
    let paragraph = heading.next_sibling().unwrap();
    let job = inline_job(paragraph, 14.0, egui::Color32::WHITE);
    assert_eq!(job.text, "bold and code");
    assert!(
        job.sections
            .iter()
            .any(|section| section.format.font_id.family == egui::FontFamily::Monospace)
    );

    let code = code_job("fn main() {}\n", "rust");
    assert_eq!(code.text, "fn main() {}\n");
    assert!(code.sections.len() > 1);
    assert_eq!(code_job("a\nb\n", "unknown-language").text, "a\nb\n");
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
    let tag_editor_id = ui.make_persistent_id(("todo-tag-editor-open", tab_id));
    let mut open_tag_editor = ui
        .ctx()
        .data_mut(|data| data.get_temp::<Option<egui::Id>>(tag_editor_id))
        .flatten();
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
                    // ponytail: cap the popup at ~5 rows, then it scrolls
                    .height(
                        all_tags.len().min(5) as f32
                            * (ui.spacing().interact_size.y + ui.spacing().item_spacing.y),
                    )
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

                        let mut remove_tag_index = None;
                        let item_tag_editor_id =
                            ui.make_persistent_id(("todo-tag-editor", item.id.as_str()));
                        let tag_editor_open = open_tag_editor == Some(item_tag_editor_id);
                        let mut focus_tag_input = false;
                        ui.horizontal_wrapped(|ui| {
                            // ponytail: pin the hover state to the idle one so chips and
                            // "#" render identically hovered or not and nothing shifts

                            // let idle = ui.visuals().widgets.inactive;
                            // ui.visuals_mut().widgets.hovered = idle;
                            // // ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                            //
                            // for (tag_index, tag) in item.tags.iter().enumerate() {
                            //     let color = tag_color(tag);
                            //     let font = egui::FontId::proportional(12.0);
                            //     let chip = egui::Button::new(
                            //         egui::RichText::new(format!("#{tag} x"))
                            //             .font(font)
                            //             .color(color),
                            //     )
                            //     .small()
                            //     .fill(color.gamma_multiply(0.15))
                            //     .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.4)))
                            //     .corner_radius(6);
                            //     if ui.add(chip).on_hover_text("Remove tag").clicked() {
                            //         remove_tag_index = Some(tag_index);
                            //     }
                            // }

                            for (tag_index, tag) in item.tags.iter().enumerate() {
                                let color = tag_color(tag);
                                // ponytail: hand-painted chip - keeps the tag color and
                                // animates on hover (tint 0.15 -> 0.35, stroke 0.4 -> 0.7);
                                let font = egui::FontId::proportional(10.0);
                                let galley =
                                    ui.painter()
                                        .layout_no_wrap(format!("#{tag} x"), font, color);
                                let pad = 6.0;
                                let (rect, response) = ui.allocate_exact_size(
                                    galley.size() + egui::vec2(pad * 2.0, pad),
                                    egui::Sense::click(),
                                );
                                let hovered = response.hovered();
                                let painter = ui.painter();
                                painter.rect_filled(
                                    rect,
                                    6.0,
                                    color.gamma_multiply(if hovered { 0.35 } else { 0.15 }),
                                );
                                painter.rect_stroke(
                                    rect,
                                    6.0,
                                    egui::Stroke::new(
                                        1.0_f32,
                                        color.gamma_multiply(if hovered { 0.7 } else { 0.4 }),
                                    ),
                                    egui::StrokeKind::Inside,
                                );
                                painter.galley(
                                    egui::pos2(
                                        rect.left() + pad,
                                        rect.center().y - galley.size().y * 0.5,
                                    ),
                                    galley,
                                    color,
                                );
                                if response.on_hover_text("Remove tag").clicked() {
                                    remove_tag_index = Some(tag_index);
                                }
                            }

                            //

                            if ui
                                .small_button("#")
                                .on_hover_text(if tag_editor_open {
                                    "Close tag editor"
                                } else {
                                    "Edit tags"
                                })
                                .clicked()
                            {
                                focus_tag_input = !tag_editor_open;
                                open_tag_editor = if tag_editor_open {
                                    None
                                } else {
                                    Some(item_tag_editor_id)
                                };
                            }

                            if tag_editor_open {
                                let draft_id =
                                    ui.make_persistent_id(("todo-tag-draft", item.id.as_str()));
                                let mut tag_draft = ui.ctx().data_mut(|data| {
                                    data.get_temp::<String>(draft_id).unwrap_or_default()
                                });
                                let response = ui.add_sized(
                                    [120.0, 24.0],
                                    egui::TextEdit::singleline(&mut tag_draft).hint_text("Add tag"),
                                );
                                if focus_tag_input {
                                    response.request_focus();
                                }
                                let enter_pressed = response.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                let add_clicked =
                                    ui.small_button("Add").on_hover_text("Add tag").clicked();
                                if (enter_pressed || add_clicked) && add_todo_tag(item, &tag_draft)
                                {
                                    tag_draft.clear();
                                    dirty = true;
                                }
                                ui.ctx()
                                    .data_mut(|data| data.insert_temp(draft_id, tag_draft));
                            }
                        });

                        if let Some(tag_index) = remove_tag_index {
                            item.tags.remove(tag_index);
                            item.touch();
                            dirty = true;
                        }
                    });
                ui.add_space(6.0);
            }
        });
    });

    ui.ctx()
        .data_mut(|data| data.insert_temp(filter_id, tag_filter));
    ui.ctx()
        .data_mut(|data| data.insert_temp(tag_editor_id, open_tag_editor));

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
