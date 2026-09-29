use std::ops::Range;
use std::sync::OnceLock;

use comrak::{
    Arena, Options,
    nodes::{ListType, Node, NodeValue},
};
use eframe::egui;
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::models::MarkdownDocument;

use super::{SURFACE_BG, SURFACE_HOVER, SURFACE_LOW, TEXT_MUTED, panel_frame};

const MARKDOWN_MIN_VISIBLE_ROWS: usize = 4;
const EDITOR_ACTIVE_TEXT: egui::Color32 = egui::Color32::from_gray(220);

#[derive(Clone, Default)]
struct SearchState {
    open: bool,
    query: String,
    current: usize,
}

impl SearchState {
    fn advance(&mut self, count: usize, previous: bool) -> bool {
        if count == 0 {
            self.current = 0;
            return false;
        }
        self.current = self.current.min(count - 1);
        self.current = if previous {
            (self.current + count - 1) % count
        } else {
            (self.current + 1) % count
        };
        true
    }
}

const SEARCH_MATCH_BG: egui::Color32 = egui::Color32::from_rgb(70, 60, 24);
const SEARCH_CURRENT_BG: egui::Color32 = egui::Color32::from_rgb(120, 88, 28);
const ICON_SIZE: f32 = 24.0;

#[derive(Clone, Copy)]
enum Icon {
    Preview,
    Search,
    Remove,
    Previous,
    Next,
    Close,
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let response = ui
        .add(
            egui::Button::new("")
                .min_size(egui::vec2(ICON_SIZE, ICON_SIZE))
                .selected(selected),
        )
        .on_hover_text(label);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let rect = egui::Rect::from_center_size(response.rect.center(), egui::vec2(14.0, 14.0));
    let at = |x: f32, y: f32| rect.min + egui::vec2(x * rect.width(), y * rect.height());
    let stroke = ui.style().interact(&response).fg_stroke;
    let painter = ui.painter();
    match icon {
        Icon::Preview => {
            for y in [-0.1, 1.1] {
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [at(0.0, 0.5), at(0.25, y), at(0.75, y), at(1.0, 0.5)],
                    false,
                    egui::Color32::TRANSPARENT,
                    stroke,
                ));
            }
            painter.circle_stroke(at(0.5, 0.5), 2.0, stroke);
        }
        Icon::Search => {
            painter.circle_stroke(at(0.4, 0.4), 4.5, stroke);
            painter.line_segment([at(0.65, 0.65), at(1.0, 1.0)], stroke);
        }
        Icon::Remove => {
            painter.rect_stroke(
                egui::Rect::from_min_max(at(0.2, 0.3), at(0.8, 1.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment([at(0.1, 0.2), at(0.9, 0.2)], stroke);
            painter.line_segment([at(0.35, 0.0), at(0.65, 0.0)], stroke);
            for x in [0.4, 0.6] {
                painter.line_segment([at(x, 0.45), at(x, 0.85)], stroke);
            }
        }
        Icon::Previous | Icon::Next => {
            let x = if matches!(icon, Icon::Previous) {
                0.25
            } else {
                0.75
            };
            painter.add(egui::Shape::line(
                vec![at(1.0 - x, 0.1), at(x, 0.5), at(1.0 - x, 0.9)],
                stroke,
            ));
        }
        Icon::Close => {
            painter.line_segment([at(0.15, 0.15), at(0.85, 0.85)], stroke);
            painter.line_segment([at(0.85, 0.15), at(0.15, 0.85)], stroke);
        }
    }
    response
}

fn gutter_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.split('\n').count().to_string(),
            egui::TextStyle::Monospace.resolve(ui.style()),
            TEXT_MUTED,
        )
        .size()
        .x
        + 12.0
}

pub(super) fn render_markdown(
    ui: &mut egui::Ui,
    documents: &mut Vec<MarkdownDocument>,
) -> (bool, Option<String>) {
    let mut dirty = false;
    let mut remove = None;
    if ui.button("+ Markdown").clicked() {
        documents.push(MarkdownDocument::new("Untitled"));
        dirty = true;
    }
    egui::ScrollArea::vertical()
        .id_salt("markdown-files-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for document in documents {
                ui.push_id(&document.file.id, |ui| {
                    let preview_id = ui.make_persistent_id("preview");
                    let mut preview =
                        ui.data(|data| data.get_temp::<bool>(preview_id).unwrap_or(false));
                    let search_id = ui.make_persistent_id("search");
                    let mut search =
                        ui.data(|data| data.get_temp::<SearchState>(search_id).unwrap_or_default());
                    let mut focus_search = false;
                    let mut reveal_match = false;
                    panel_frame(SURFACE_LOW).show(ui, |ui| {
                        let gutter_width = gutter_width(ui, &document.text);
                        ui.horizontal(|ui| {
                            let label = if document.file.collapsed {
                                "Expand"
                            } else {
                                "Collapse"
                            };
                            let (_, response) = ui.allocate_exact_size(
                                egui::vec2(gutter_width, ICON_SIZE),
                                egui::Sense::click(),
                            );
                            let response = response.on_hover_text(label);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::CollapsingHeader,
                                    ui.is_enabled(),
                                    label,
                                )
                            });
                            let mut arrow = response.clone();
                            arrow.rect = egui::Rect::from_center_size(
                                response.rect.center(),
                                egui::vec2(14.0, 14.0),
                            );
                            egui::collapsing_header::paint_default_icon(
                                ui,
                                if document.file.collapsed { 0.0 } else { 1.0 },
                                &arrow,
                            );
                            if response.clicked() {
                                document.file.collapsed = !document.file.collapsed;
                                dirty = true;
                            }
                            let title_width = ui.available_width()
                                - 3.0 * (ICON_SIZE + ui.spacing().item_spacing.x);
                            dirty |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut document.file.title)
                                        .id_salt("title")
                                        .font(egui::TextStyle::Monospace)
                                        .frame(
                                            egui::Frame::new()
                                                .fill(ui.visuals().text_edit_bg_color())
                                                .inner_margin(egui::Margin::symmetric(4, 2)),
                                        )
                                        .hint_text("Untitled")
                                        .desired_width(title_width.max(40.0)),
                                )
                                .changed();
                            if icon_button(ui, Icon::Preview, "Preview", preview).clicked() {
                                preview = !preview;
                                ui.data_mut(|data| data.insert_temp(preview_id, preview));
                                if preview {
                                    search.open = false;
                                }
                                if document.file.collapsed {
                                    document.file.collapsed = false;
                                    dirty = true;
                                }
                            }
                            if icon_button(ui, Icon::Search, "Search Markdown source", search.open)
                                .clicked()
                            {
                                search.open = !search.open;
                                if search.open {
                                    preview = false;
                                    ui.data_mut(|data| data.insert_temp(preview_id, false));
                                    focus_search = true;
                                    reveal_match = true;
                                    if document.file.collapsed {
                                        document.file.collapsed = false;
                                        dirty = true;
                                    }
                                }
                            }
                            if icon_button(ui, Icon::Remove, "Remove Markdown file", false)
                                .clicked()
                            {
                                remove = Some(document.file.id.clone());
                            }
                        });
                        if !document.file.collapsed {
                            if search.open {
                                reveal_match |= render_search_bar(
                                    ui,
                                    &document.text,
                                    &mut search,
                                    search_id.with("query"),
                                    focus_search,
                                );
                            }
                            ui.add_space(4.0);
                            if preview {
                                render_markdown_preview(ui, &document.text);
                            } else {
                                dirty |= render_numbered_editor(
                                    ui,
                                    &mut document.text,
                                    &search,
                                    reveal_match,
                                );
                            }
                        }
                    });
                    ui.data_mut(|data| data.insert_temp(search_id, search));
                });
            }
        });
    (dirty, remove)
}

fn render_search_bar(
    ui: &mut egui::Ui,
    text: &str,
    search: &mut SearchState,
    id: egui::Id,
    focus: bool,
) -> bool {
    ui.horizontal(|ui| {
        ui.allocate_exact_size(
            egui::vec2(gutter_width(ui, text), 0.0),
            egui::Sense::hover(),
        );
        let count = search_ranges(text, &search.query).len();
        let counter = format!(
            "{}/{}",
            if count == 0 {
                0
            } else {
                search.current.min(count - 1) + 1
            },
            count
        );
        let counter_width = ui
            .painter()
            .layout_no_wrap(
                counter,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x;
        let controls_width = 3.0 * ICON_SIZE + 4.0 * ui.spacing().item_spacing.x + counter_width;
        let input = ui.add(
            egui::TextEdit::singleline(&mut search.query)
                .id(id)
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().text_edit_bg_color())
                        .inner_margin(egui::Margin::symmetric(4, 2)),
                )
                .hint_text("Find text (case-sensitive)")
                .desired_width((ui.available_width() - controls_width).max(40.0)),
        );
        if focus {
            input.request_focus();
        }
        let mut reveal = input.changed();
        if reveal {
            search.current = 0;
        }
        let count = search_ranges(text, &search.query).len();
        search.current = search.current.min(count.saturating_sub(1));
        let (enter, previous, escape) = ui.input(|input_state| {
            (
                input_state.key_pressed(egui::Key::Enter),
                input_state.modifiers.shift,
                input_state.key_pressed(egui::Key::Escape),
            )
        });
        if input.has_focus() || input.lost_focus() {
            if enter {
                reveal |= search.advance(count, previous);
                input.request_focus();
            }
            if escape {
                search.open = false;
            }
        }
        ui.add_enabled_ui(count > 0, |ui| {
            if icon_button(ui, Icon::Previous, "Previous match (Shift+Enter)", false).clicked() {
                reveal |= search.advance(count, true);
            }
            if icon_button(ui, Icon::Next, "Next match (Enter)", false).clicked() {
                reveal |= search.advance(count, false);
            }
        });
        ui.label(format!(
            "{}/{}",
            if count == 0 { 0 } else { search.current + 1 },
            count
        ));
        if icon_button(ui, Icon::Close, "Close search", false).clicked() {
            search.open = false;
        }
        reveal
    })
    .inner
}

fn search_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    text.match_indices(query)
        .map(|(start, matched)| start..start + matched.len())
        .collect()
}

fn search_job(text: &str, search: &SearchState, format: egui::TextFormat) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let matches = search_ranges(text, if search.open { &search.query } else { "" });
    let current = search.current.min(matches.len().saturating_sub(1));
    let mut end = 0;
    for (index, range) in matches.iter().enumerate() {
        job.append(&text[end..range.start], 0.0, format.clone());
        let mut matched = format.clone();
        matched.background = if index == current {
            SEARCH_CURRENT_BG
        } else {
            SEARCH_MATCH_BG
        };
        job.append(&text[range.clone()], 0.0, matched);
        end = range.end;
    }
    job.append(&text[end..], 0.0, format);
    job
}

fn render_numbered_editor(
    ui: &mut egui::Ui,
    text: &mut String,
    search: &SearchState,
    reveal_match: bool,
) -> bool {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let gutter_width = gutter_width(ui, text);
    let editor_id = ui.make_persistent_id("body");
    ui.horizontal_top(|ui| {
        ui.visuals_mut().text_cursor.stroke =
            egui::Stroke::new(2.0_f32, egui::Color32::from_gray(200));
        let (gutter, _) =
            ui.allocate_exact_size(egui::vec2(gutter_width, 0.0), egui::Sense::hover());
        let desired_rows = markdown_editor_desired_rows(text);
        // Reserve paint slots behind the text, selection, and caret.
        let background = ui.painter().add(egui::Shape::Noop);
        let highlight = ui.painter().add(egui::Shape::Noop);
        let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
            let mut job = search_job(
                buffer.as_str(),
                search,
                egui::TextFormat::simple(font.clone(), ui.visuals().text_color()),
            );
            job.wrap.max_width = width;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let output = egui::TextEdit::multiline(text)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2)))
            .id(editor_id)
            .font(font.clone())
            .layouter(&mut layouter)
            .hint_text("Write markdown...")
            .desired_width(f32::INFINITY)
            .desired_rows(desired_rows)
            .show(ui);
        //
        if let Some(range) = output.cursor_range {
            let row = output
                .galley
                .pos_from_cursor(range.primary)
                .translate(output.galley_pos.to_vec2());
            // Trim egui's extra caret height, preserving its native blinking and focus handling.
            ui.ctx().graphics_mut(|graphics| {
                let shapes = graphics.entry(ui.layer_id());
                for index in highlight.0 + 1..shapes.next_idx().0 {
                    shapes.mutate_shape(egui::layers::ShapeIdx(index), |shape| {
                        if let egui::Shape::LineSegment { points, stroke } = &mut shape.shape
                            && *stroke == ui.visuals().text_cursor.stroke
                        {
                            points[0].y = row.top();
                            points[1].y = row.bottom();
                        }
                    });
                }
            });
        }
        //
        let editor_rect = output.response.rect.union(gutter);
        ui.painter().set(
            background,
            egui::Shape::rect_filled(output.response.rect, 2.0, ui.visuals().text_edit_bg_color()),
        );
        if reveal_match && search.open {
            let matches = search_ranges(text, &search.query);
            if let Some(range) = matches.get(search.current.min(matches.len().saturating_sub(1))) {
                let cursor = egui::text::CCursor::new(text[..range.start].chars().count());
                let rect = output
                    .galley
                    .pos_from_cursor(cursor)
                    .translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(rect, Some(egui::Align::Center));
            }
        }
        let active_line = output
            .cursor_range
            .filter(|_| output.response.has_focus())
            .map(|range| active_line_number(&output.galley, range.primary));
        let positions = line_number_positions(&output.galley);
        for (index, &(number, y)) in positions.iter().enumerate() {
            let active = active_line == Some(number);
            if active {
                let bottom = positions
                    .get(index + 1)
                    .map_or(output.galley.rect.bottom(), |(_, y)| *y);
                let rect = egui::Rect::from_min_max(
                    egui::pos2(editor_rect.left(), output.galley_pos.y + y),
                    egui::pos2(editor_rect.right(), output.galley_pos.y + bottom),
                )
                .intersect(editor_rect);
                ui.painter().set(
                    highlight,
                    egui::Shape::rect_filled(rect, 0.0, SURFACE_HOVER),
                );
            }
            ui.painter().text(
                egui::pos2(gutter.right() - 4.0, output.galley_pos.y + y),
                egui::Align2::RIGHT_TOP,
                number.to_string(),
                font.clone(),
                if active {
                    EDITOR_ACTIVE_TEXT
                } else {
                    TEXT_MUTED
                },
            );
        }
        output.response.changed()
    })
    .inner
}

fn active_line_number(galley: &egui::Galley, cursor: egui::text::CCursor) -> usize {
    let row = galley.layout_from_cursor(cursor).row;
    1 + galley.rows[..row]
        .iter()
        .filter(|row| row.ends_with_newline)
        .count()
}

fn line_number_positions(galley: &egui::Galley) -> Vec<(usize, f32)> {
    let mut numbers = Vec::new();
    let mut start_of_line = true;
    for row in &galley.rows {
        if start_of_line {
            numbers.push((numbers.len() + 1, row.pos.y));
        }
        start_of_line = row.ends_with_newline;
    }
    numbers
}

const PREVIEW_FONT: &str = "markdown-regular";
const PREVIEW_BOLD: &str = "markdown-bold";
const PREVIEW_TEXT: egui::Color32 = egui::Color32::from_rgb(206, 211, 215);
const REGULAR_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

pub(super) fn configure_preview(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (PREVIEW_FONT, REGULAR_FONT),
        (
            PREVIEW_BOLD,
            include_bytes!("../../assets/fonts/NotoSans-Bold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
        let mut family = fonts.families[&egui::FontFamily::Proportional].clone();
        family.insert(0, name.into());
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), family);
    }
    context.set_fonts(fonts);
    egui_extras::install_image_loaders(context);
}

fn render_markdown_preview(ui: &mut egui::Ui, markdown: &str) {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    let document = comrak::parse_document(&arena, markdown, &options);
    ui.scope(|ui| {
        ui.visuals_mut().override_text_color = Some(PREVIEW_TEXT);
        ui.spacing_mut().item_spacing.y = 4.0;
        for child in document.children() {
            render_markdown_block(ui, child);
        }
    });
}

fn render_markdown_block(ui: &mut egui::Ui, node: Node<'_>) {
    match &node.data().value {
        NodeValue::Heading(heading) => {
            let size = match heading.level {
                1 => 28.0,
                2 => 22.0,
                3 => 18.0,
                _ => 16.0,
            };
            ui.add_space(if node.previous_sibling().is_some() {
                14.0
            } else {
                4.0
            });
            render_inline(ui, node, size);
            if heading.level <= 2 {
                ui.separator();
            }
            ui.add_space(6.0);
        }
        NodeValue::Paragraph | NodeValue::TableCell => {
            render_inline(ui, node, 16.0);
            // Tight list items and table cells should not get paragraph-sized gaps.
            if matches!(node.data().value, NodeValue::Paragraph)
                && !node.parent().is_some_and(|parent| {
                    matches!(
                        parent.data().value,
                        NodeValue::Item(_) | NodeValue::TaskItem(_)
                    )
                })
            {
                ui.add_space(10.0);
            }
        }
        NodeValue::CodeBlock(block) => {
            let language = block.info.split_whitespace().next().unwrap_or("");
            let width = ui.available_width();
            egui::Frame::new()
                .fill(SURFACE_BG)
                .stroke(egui::Stroke::new(1.0_f32, super::STROKE))
                .corner_radius(5)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: 12,
                    bottom: 4,
                })
                .show(ui, |ui| {
                    ui.set_min_width((width - 26.0).max(0.0));
                    ui.spacing_mut().scroll = egui::style::ScrollStyle {
                        bar_width: 4.0,
                        floating_width: 4.0,
                        content_margin: egui::Margin {
                            bottom: 4,
                            ..egui::Margin::ZERO
                        },
                        foreground_color: false,
                        ..egui::style::ScrollStyle::floating()
                    };
                    ui.visuals_mut().extreme_bg_color = SURFACE_BG;
                    let widgets = &mut ui.visuals_mut().widgets;
                    widgets.inactive.bg_fill = egui::Color32::from_gray(95);
                    widgets.hovered.bg_fill = egui::Color32::from_gray(130);
                    widgets.active.bg_fill = egui::Color32::from_gray(155);
                    egui::ScrollArea::horizontal()
                        .id_salt(("markdown-code", node.data().sourcepos.start.line))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(code_job(
                                    block.literal.trim_end_matches('\n'),
                                    language,
                                ))
                                .extend()
                                .selectable(true),
                            );
                        });
                });
            ui.add_space(10.0);
        }
        NodeValue::List(list) => {
            for (index, child) in node.children().enumerate() {
                let marker = if let NodeValue::TaskItem(task) = &child.data().value {
                    if task.symbol.is_some() { "☑" } else { "☐" }.to_owned()
                } else {
                    match list.list_type {
                        ListType::Ordered => format!("{}.", list.start + index),
                        ListType::Bullet => "•".to_owned(),
                    }
                };
                ui.horizontal_top(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(marker).size(16.0));
                    ui.vertical(|ui| {
                        for item in child.children() {
                            render_markdown_block(ui, item);
                        }
                    });
                });
            }
            ui.add_space(10.0);
        }
        NodeValue::BlockQuote => {
            egui::Frame::new()
                .fill(SURFACE_BG)
                .inner_margin(10)
                .show(ui, |ui| {
                    for child in node.children() {
                        render_markdown_block(ui, child);
                    }
                });
        }
        NodeValue::Table(_) => {
            egui::Grid::new(("markdown-table", node.data().sourcepos.start.line))
                .striped(true)
                .show(ui, |ui| {
                    for row in node.children() {
                        for cell in row.children() {
                            render_markdown_block(ui, cell);
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(10.0);
        }
        NodeValue::ThematicBreak => {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
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

fn render_inline(ui: &mut egui::Ui, node: Node<'_>, size: f32) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.spacing_mut().interact_size.y = size * 1.5;
        ui.horizontal_wrapped(|ui| {
            let mut job = egui::text::LayoutJob::default();
            append_inline(
                ui,
                node,
                &mut job,
                egui::TextFormat {
                    font_id: egui::FontId::new(size, egui::FontFamily::Name(PREVIEW_FONT.into())),
                    color: PREVIEW_TEXT,
                    line_height: Some(size * 1.5),
                    ..Default::default()
                },
                None,
            );
            flush_inline(ui, &mut job, None);
        });
    });
}

fn flush_inline(ui: &mut egui::Ui, job: &mut egui::text::LayoutJob, link: Option<&str>) {
    if !job.text.is_empty() {
        let job = std::mem::take(job);
        if let Some(url) = link {
            ui.hyperlink_to(job, url);
        } else {
            ui.add(egui::Label::new(job).wrap().selectable(true));
        }
    }
}

fn safe_link(url: &str) -> bool {
    !url.chars().any(char::is_control)
        && ["https://", "http://", "mailto:"].iter().any(|scheme| {
            url.strip_prefix(scheme)
                .is_some_and(|rest| !rest.is_empty())
        })
}

fn append_inline(
    ui: &mut egui::Ui,
    node: Node<'_>,
    job: &mut egui::text::LayoutJob,
    mut format: egui::TextFormat,
    link: Option<&str>,
) {
    match &node.data().value {
        NodeValue::Text(text) => job.append(text, 0.0, format.clone()),
        NodeValue::Code(code) => {
            format.font_id = egui::FontId::monospace(format.font_id.size * 0.9);
            format.background = SURFACE_HOVER;
            job.append(&code.literal, 0.0, format.clone());
        }
        NodeValue::SoftBreak => job.append(" ", 0.0, format.clone()),
        NodeValue::LineBreak => job.append("\n", 0.0, format.clone()),
        NodeValue::Strong | NodeValue::Heading(_) => {
            format.font_id.family = egui::FontFamily::Name(PREVIEW_BOLD.into());
        }
        NodeValue::Emph => format.italics = true,
        NodeValue::Strikethrough => format.strikethrough = egui::Stroke::new(1.0_f32, format.color),
        NodeValue::Link(target) => {
            flush_inline(ui, job, link);
            let target = safe_link(&target.url).then_some(target.url.as_str());
            if target.is_some() {
                format.color = super::ACCENT;
            }
            for child in node.children() {
                append_inline(ui, child, job, format.clone(), target);
            }
            flush_inline(ui, job, target);
            return;
        }
        NodeValue::Image(image) => {
            flush_inline(ui, job, link);
            let alt: String = node
                .descendants()
                .filter_map(|child| {
                    if let NodeValue::Text(text) = &child.data().value {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let response = render_preview_image(ui, &image.url, &alt);
            if let Some(url) = link {
                let response = response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(url);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Link, ui.is_enabled(), &alt)
                });
                if response.clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }
            return;
        }
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
        append_inline(ui, child, job, format.clone(), link);
    }
}

fn render_preview_image(ui: &mut egui::Ui, url: &str, alt: &str) -> egui::Response {
    if !safe_link(url) || url.starts_with("mailto:") {
        return ui
            .label(alt)
            .on_hover_text("Only HTTP(S) images are supported");
    }
    // Inspect MIME rather than the URL extension: Shields badges have no .svg suffix.
    match ui.ctx().try_load_bytes(url) {
        Ok(egui::load::BytesPoll::Ready { bytes, mime, .. }) => {
            if mime.as_deref().is_some_and(|mime| mime.contains("svg"))
                || bytes[..bytes.len().min(1024)]
                    .windows(4)
                    .any(|window| window == b"<svg")
            {
                // ponytail: SVG textures are cached for the session; add eviction for image-heavy workspaces.
                let id = egui::Id::new(("markdown-svg", url));
                let cached = ui.data(|data| {
                    data.get_temp::<Result<(egui::TextureHandle, egui::Vec2), String>>(id)
                });
                let image = cached.unwrap_or_else(|| {
                    let result = rasterize_svg(&bytes).map(|image| {
                        let size = image.source_size;
                        (
                            ui.ctx()
                                .load_texture(url, image, egui::TextureOptions::LINEAR),
                            size,
                        )
                    });
                    ui.data_mut(|data| data.insert_temp(id, result.clone()));
                    result
                });
                match image {
                    Ok((texture, size)) => ui.add(
                        egui::Image::from_texture((texture.id(), size))
                            .max_width(ui.available_width())
                            .alt_text(alt),
                    ),
                    Err(error) => ui.label(alt).on_hover_text(error),
                }
            } else {
                ui.add(
                    egui::Image::new(url)
                        .max_width(ui.available_width())
                        .alt_text(alt),
                )
            }
        }
        Ok(egui::load::BytesPoll::Pending { .. }) => ui.label(alt).on_hover_text("Loading image…"),
        Err(error) => ui.label(alt).on_hover_text(error.to_string()),
    }
}

fn rasterize_svg(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    static OPTIONS: OnceLock<resvg::usvg::Options<'static>> = OnceLock::new();
    let options = OPTIONS.get_or_init(|| {
        let mut options = resvg::usvg::Options {
            font_family: "Noto Sans".into(),
            ..Default::default()
        };
        options.fontdb_mut().load_font_data(REGULAR_FONT.to_vec());
        options.fontdb_mut().set_sans_serif_family("Noto Sans");
        options.fontdb_mut().set_serif_family("Noto Sans");
        // Remote SVGs must not read local files, including nested image resources.
        options.image_href_resolver.resolve_string = Box::new(|_, _| None);
        options.image_href_resolver.resolve_data = Box::new(|_, _, _| None);
        options
    });
    let tree = resvg::usvg::Tree::from_data(bytes, options).map_err(|error| error.to_string())?;
    let size = egui::vec2(tree.size().width(), tree.size().height());
    let scale = 2.0_f32.min(4096.0 / size.x.max(size.y));
    let mut pixels = resvg::tiny_skia::Pixmap::new(
        (size.x * scale).ceil() as u32,
        (size.y * scale).ceil() as u32,
    )
    .ok_or_else(|| "Invalid SVG dimensions".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    Ok(egui::ColorImage::from_rgba_premultiplied(
        [pixels.width() as usize, pixels.height() as usize],
        pixels.data(),
    )
    .with_source_size(size))
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
                            font_id: egui::FontId::monospace(15.0),
                            line_height: Some(22.0),
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
                    font_id: egui::FontId::monospace(15.0),
                    line_height: Some(22.0),
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
    let context = egui::Context::default();
    super::configure_theme(&context);
    configure_preview(&context);
    let badge = br#"<svg xmlns="http://www.w3.org/2000/svg" width="90" height="20"><text x="4" y="15" fill="white" font-family="Verdana" font-size="12">passing</text></svg>"#;
    let pixels = rasterize_svg(badge).unwrap();
    assert_eq!(pixels.source_size, egui::vec2(90.0, 20.0));
    assert!(
        pixels.pixels.iter().any(|pixel| pixel.a() != 0),
        "SVG text must render without system fonts"
    );
    assert!(rasterize_svg(b"not SVG").is_err());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="{}" width="20" height="20"/></svg>"#,
        path.display()
    );
    assert!(
        rasterize_svg(svg.as_bytes())
            .unwrap()
            .pixels
            .iter()
            .all(|pixel| pixel.a() == 0)
    );
    assert!(safe_link("https://example.com"));
    assert!(!safe_link("javascript:alert(1)"));
    assert!(!safe_link("file:///etc/passwd"));
    assert!(!safe_link("https://example.com\n"));
    context.include_bytes("https://example.com/badge", badge.as_slice());
    // Prefer the in-memory fixture over HTTP so this check never uses the network.
    context.add_bytes_loader(context.loaders().include.clone());
    let markdown = "# Title\n\n[![Badge](https://example.com/badge)](https://example.com/release)\n[![Badge](https://example.com/badge)](https://example.com/release)\n\nOne soft\nline with **bold** and `code`.\n\n## Quick Start\n\n```bash\ncargo run\n```\n\n* `-x`  extract the files  \n* archive‑creation\n\nhard  \nbreak\n\n---\n";
    let mut width = 0.0;
    let mut frame = |events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                width = ui.available_width();
                render_markdown_preview(ui, markdown);
            },
        )
    };
    frame(vec![]);
    let output = frame(vec![]);
    let jobs: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some(&text.galley.job)
            } else {
                None
            }
        })
        .collect();
    let paragraph = jobs
        .iter()
        .find(|job| job.text == "One soft line with bold and code.")
        .unwrap();
    assert!(
        paragraph
            .sections
            .iter()
            .any(
                |section| &paragraph.text[section.byte_range.clone()] == "bold"
                    && section.format.font_id.family == egui::FontFamily::Name(PREVIEW_BOLD.into())
            )
    );
    assert!(
        paragraph
            .sections
            .iter()
            .any(
                |section| &paragraph.text[section.byte_range.clone()] == "code"
                    && section.format.background == SURFACE_HOVER
            )
    );
    assert!(jobs.iter().any(|job| job.text == "hard\nbreak"));
    assert!(jobs.iter().any(|job| job.text == "-x  extract the files"));
    assert!(jobs.iter().any(|job| job.text == "archive‑creation"));
    let heading = jobs.iter().find(|job| job.text == "Title").unwrap();
    assert_eq!(heading.sections[0].format.font_id.size, 28.0);
    assert_eq!(
        heading.sections[0].format.font_id.family,
        egui::FontFamily::Name(PREVIEW_BOLD.into())
    );
    let badge_texture = context
        .data(|data| {
            data.get_temp::<Result<(egui::TextureHandle, egui::Vec2), String>>(egui::Id::new((
                "markdown-svg",
                "https://example.com/badge",
            )))
        })
        .unwrap()
        .unwrap()
        .0;
    let badges: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Rect(rect) = &shape.shape {
                (rect.fill_texture_id() == badge_texture.id()).then_some(rect.rect)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(badges.len(), 2);
    assert_eq!(badges[0].top(), badges[1].top());
    assert!(badges[1].left() >= badges[0].right());
    let pos = badges[0].center();
    frame(vec![egui::Event::PointerMoved(pos)]);
    frame(vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    let clicked = frame(vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(clicked.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == "https://example.com/release")));
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == SURFACE_BG && (rect.rect.width() - width).abs() < 1.0)), "code block must span the preview width");

    let code = code_job("fn main() {}\n", "rust");
    assert_eq!(code.text, "fn main() {}\n");
    assert!(code.sections.len() > 1);
    assert_eq!(code_job("a\nb\n", "unknown-language").text, "a\nb\n");
}

pub(super) fn markdown_editor_desired_rows(markdown: &str) -> usize {
    markdown.split('\n').count().max(MARKDOWN_MIN_VISIBLE_ROWS)
}

#[cfg(test)]
#[test]
fn line_numbers_follow_logical_lines_including_empty_and_wrapped_lines() {
    let context = egui::Context::default();
    let _ = context.run_ui(egui::RawInput::default(), |ui| {
        for text in [
            "",
            "a\n\n",
            "a very long line that wraps across several rows\nnext\n",
            "日本語\nnext\n",
        ] {
            let galley = ui.painter().layout(
                text.to_owned(),
                egui::FontId::monospace(14.0),
                egui::Color32::WHITE,
                60.0,
            );
            let positions = line_number_positions(&galley);
            assert_eq!(positions.len(), text.split('\n').count());
            for index in 0..=text.chars().count() {
                let expected = 1 + text.chars().take(index).filter(|ch| *ch == '\n').count();
                assert_eq!(
                    active_line_number(&galley, egui::text::CCursor::new(index)),
                    expected
                );
            }
            for (index, (number, y)) in positions.iter().enumerate() {
                assert_eq!(*number, index + 1);
                if index > 0 {
                    assert!(*y > positions[index - 1].1);
                }
            }
            if text.starts_with("a very") {
                assert!(galley.rows.len() > positions.len());
                assert!(positions[1].1 > galley.rows[1].pos.y);
            }
        }
        let mut documents = vec![
            MarkdownDocument::new("First"),
            MarkdownDocument::new("Second"),
        ];
        documents[0].text = "body\nwith two lines".to_owned();
        documents[1].file.collapsed = true;
        assert_eq!(render_markdown(ui, &mut documents), (false, None));
    });
}

#[cfg(test)]
#[test]
fn editor_caret_matches_text_row_height() {
    for text in [
        "",
        "first\nsecond",
        "a long line that wraps across multiple rows",
        "last\n",
    ] {
        let context = egui::Context::default();
        let mut text = text.to_owned();
        let mut row_height = 0.0;
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(120.0);
            ui.visuals_mut().text_cursor.blink = false;
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            row_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
            let id = ui.make_persistent_id("body");
            ui.memory_mut(|memory| memory.request_focus(id));
            let mut state = egui::text_edit::TextEditState::default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(text.chars().count()),
                )));
            state.store(ui.ctx(), id);
            render_numbered_editor(ui, &mut text, &SearchState::default(), false);
        });
        let caret = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::LineSegment { points, stroke } = &shape.shape {
                    (stroke.width == 2.0 && stroke.color == egui::Color32::from_gray(160))
                        .then_some(points)
                } else {
                    None
                }
            })
            .expect("focused editor paints a caret");
        assert!(
            (caret[1].y - caret[0].y - row_height.round()).abs() < 0.1,
            "text {text:?}: caret height {}, row height {row_height}",
            caret[1].y - caret[0].y
        );
    }
}

#[cfg(test)]
#[test]
fn search_handles_unicode_navigation_and_closed_highlights() {
    let text = "é 猫\n猫 é";
    assert_eq!(search_ranges(text, "猫"), vec![3..6, 7..10]);
    assert!(search_ranges(text, "").is_empty());
    assert!(search_ranges(text, "missing").is_empty());
    assert!(search_ranges("Note", "note").is_empty());
    assert_eq!(search_ranges("ababa", "aba"), vec![0..3]);
    let mut search = SearchState {
        open: true,
        query: "猫".to_owned(),
        current: 0,
    };
    assert!(search.advance(2, true));
    assert_eq!(search.current, 1);
    let job = search_job(text, &search, egui::TextFormat::default());
    assert_eq!(job.text, text);
    let highlighted: Vec<_> = job
        .sections
        .iter()
        .filter(|section| section.format.background != egui::Color32::TRANSPARENT)
        .collect();
    assert_eq!(highlighted.len(), 2);
    assert_eq!(highlighted[0].byte_range, 3..6);
    assert_eq!(highlighted[0].format.background, SEARCH_MATCH_BG);
    assert_eq!(highlighted[1].format.background, SEARCH_CURRENT_BG);
    assert!(search.advance(2, false));
    assert_eq!(search.current, 0);
    search.current = 99;
    assert!(search.advance(1, false));
    assert_eq!(search.current, 0);
    assert!(!search.advance(0, false));
    search.open = false;
    assert!(
        search_job(text, &search, egui::TextFormat::default())
            .sections
            .iter()
            .all(|section| section.format.background == egui::Color32::TRANSPARENT)
    );
}

#[cfg(test)]
#[test]
fn search_is_per_file_and_title_aligns_with_editor_text() {
    let context = egui::Context::default();
    super::configure_theme(&context);
    let mut first = MarkdownDocument::new("First title");
    first.text = "needle\nneedle\n".to_owned();
    let mut second = MarkdownDocument::new("Second title");
    second.text = "needle in another file".to_owned();
    let mut documents = vec![first, second];
    let original = documents.clone();
    let mut frame = |events: Vec<egui::Event>| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                assert_eq!(render_markdown(ui, &mut documents), (false, None));
            },
        )
    };
    let output = frame(vec![]);
    let text_x = |text: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(shape) = &shape.shape {
                    (shape.galley.job.text == text).then_some(shape.pos.x)
                } else {
                    None
                }
            })
            .expect("text painted")
    };
    assert_eq!(text_x("First title"), text_x("needle\nneedle\n"));
    assert_eq!(text_x("Second title"), text_x("needle in another file"));
    let search_position = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Circle(circle) = &shape.shape {
                (circle.radius == 4.5).then_some(circle.center)
            } else {
                None
            }
        })
        .expect("search icon painted");
    frame(vec![egui::Event::PointerMoved(search_position)]);
    for pressed in [true, false] {
        frame(vec![egui::Event::PointerButton {
            pos: search_position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    frame(vec![egui::Event::Text("needle".to_owned())]);
    let output = frame(vec![]);
    let search_x = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(shape) = &shape.shape {
                (shape.galley.job.text == "needle").then_some(shape.pos.x)
            } else {
                None
            }
        })
        .expect("search input painted");
    assert_eq!(search_x, text_x("First title"));
    let focused = context.memory(|memory| memory.focused()).unwrap();
    let input_rect = context.read_response(focused).unwrap().rect;
    let editor_right = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Rect(rect) = &shape.shape {
                (rect.fill == context.global_style().visuals.text_edit_bg_color()
                    && rect.rect.height() > ICON_SIZE)
                    .then_some(rect.rect.right())
            } else {
                None
            }
        })
        .expect("editor background painted");
    let controls_right = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Rect(rect) = &shape.shape {
                (rect.rect.left() > input_rect.right()
                    && input_rect.y_range().contains(rect.rect.center().y))
                .then_some(rect.rect.right())
            } else {
                None
            }
        })
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (controls_right - editor_right).abs() < 1.0,
        "controls end at {controls_right}, editor ends at {editor_right}"
    );
    let jobs: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(shape) = &shape.shape {
                Some(&shape.galley.job)
            } else {
                None
            }
        })
        .collect();
    assert!(jobs.iter().any(|job| job.text == "1/2"));
    let first = jobs
        .iter()
        .find(|job| job.text == "needle\nneedle\n")
        .unwrap();
    assert_eq!(
        first
            .sections
            .iter()
            .filter(|section| section.format.background != egui::Color32::TRANSPARENT)
            .count(),
        2
    );
    let second = jobs
        .iter()
        .find(|job| job.text == "needle in another file")
        .unwrap();
    assert!(
        second
            .sections
            .iter()
            .all(|section| section.format.background == egui::Color32::TRANSPARENT)
    );
    let output = frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(output.shapes.iter().any(
        |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "2/2")
    ));
    assert_eq!(documents, original);
}
