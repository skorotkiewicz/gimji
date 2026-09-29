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

use super::{SURFACE_BG, SURFACE_LOW, TEXT_MUTED, panel_frame};

const MARKDOWN_MIN_VISIBLE_ROWS: usize = 4;

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
                    panel_frame(SURFACE_LOW).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .small_button(if document.file.collapsed {
                                    "▶"
                                } else {
                                    "▼"
                                })
                                .on_hover_text(if document.file.collapsed {
                                    "Expand"
                                } else {
                                    "Collapse"
                                })
                                .clicked()
                            {
                                document.file.collapsed = !document.file.collapsed;
                                dirty = true;
                            }
                            dirty |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut document.file.title)
                                        .id_salt("title")
                                        .hint_text("Untitled")
                                        .desired_width((ui.available_width() - 160.0).max(80.0)),
                                )
                                .changed();
                            if ui.selectable_label(preview, "Preview").clicked() {
                                preview = !preview;
                                ui.data_mut(|data| data.insert_temp(preview_id, preview));
                                if document.file.collapsed {
                                    document.file.collapsed = false;
                                    dirty = true;
                                }
                            }
                            if ui.button("Remove").clicked() {
                                remove = Some(document.file.id.clone());
                            }
                        });
                        if !document.file.collapsed {
                            ui.add_space(4.0);
                            if preview {
                                render_markdown_preview(ui, &document.text);
                            } else {
                                dirty |= render_numbered_editor(ui, &mut document.text);
                            }
                        }
                    });
                });
            }
        });
    (dirty, remove)
}

fn render_numbered_editor(ui: &mut egui::Ui, text: &mut String) -> bool {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let line_count = text.split('\n').count();
    let gutter_width = ui
        .painter()
        .layout_no_wrap(line_count.to_string(), font.clone(), TEXT_MUTED)
        .size()
        .x
        + 12.0;
    ui.horizontal_top(|ui| {
        let (gutter, _) =
            ui.allocate_exact_size(egui::vec2(gutter_width, 0.0), egui::Sense::hover());
        let desired_rows = markdown_editor_desired_rows(text);
        let output = egui::TextEdit::multiline(text)
            .id_salt("body")
            .font(font.clone())
            .hint_text("Write markdown...")
            .desired_width(f32::INFINITY)
            .desired_rows(desired_rows)
            .show(ui);
        for (number, y) in line_number_positions(&output.galley) {
            ui.painter().text(
                egui::pos2(gutter.right() - 4.0, output.galley_pos.y + y),
                egui::Align2::RIGHT_TOP,
                number.to_string(),
                font.clone(),
                TEXT_MUTED,
            );
        }
        output.response.changed()
    })
    .inner
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
        ] {
            let galley = ui.painter().layout(
                text.to_owned(),
                egui::FontId::monospace(14.0),
                egui::Color32::WHITE,
                60.0,
            );
            let positions = line_number_positions(&galley);
            assert_eq!(positions.len(), text.split('\n').count());
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
