use std::sync::OnceLock;

use comrak::{
    Arena, Options,
    nodes::{ListType, Node, NodeValue},
};
use eframe::egui;
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use super::{SURFACE_BG, SURFACE_LOW, panel_frame};

const MARKDOWN_MIN_VISIBLE_ROWS: usize = 24;

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
