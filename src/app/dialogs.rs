use eframe::egui;

use super::{ConfirmAction, GimjiApp};

impl GimjiApp {
    pub(super) fn render_confirm(&mut self, context: &egui::Context) {
        let Some(action) = &self.pending_confirm else {
            return;
        };

        let message = match action {
            ConfirmAction::DeleteNote(_) => {
                "Delete this note from config? Content files stay on disk."
            }
            ConfirmAction::DeleteTab(_) => {
                "Delete this tab from config? Content file stays on disk."
            }
            #[cfg(feature = "s3")]
            ConfirmAction::RestoreWorkspaceFromS3 => {
                "Restore this workspace from S3? Local config and content files will be overwritten."
            }
        };
        let confirm_label = match action {
            #[cfg(feature = "s3")]
            ConfirmAction::RestoreWorkspaceFromS3 => "Restore",
            _ => confirm_button_label_for_delete(),
        };
        let show_remove_local_files = matches!(
            action,
            ConfirmAction::DeleteNote(_) | ConfirmAction::DeleteTab(_)
        );

        egui::Window::new("Confirm")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(message);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if ui.button("Cancel").clicked() {
                        self.pending_confirm = None;
                        self.remove_local_files_on_delete = false;
                    }
                    if ui.button(confirm_label).clicked() {
                        self.confirm_action();
                    }
                });
                if show_remove_local_files {
                    ui.checkbox(
                        &mut self.remove_local_files_on_delete,
                        "Remove local content files",
                    );
                }
            });
    }

    pub(super) fn render_message(&mut self, context: &egui::Context) {
        let Some(message) = self.message.clone() else {
            return;
        };

        egui::Window::new("Status")
            .collapsible(false)
            .resizable(true)
            .show(context, |ui| {
                ui.label(message);
                ui.add_space(12.0);
                if ui.button("OK").clicked() {
                    self.message = None;
                }
            });
    }
}

pub(super) fn confirm_button_label_for_delete() -> &'static str {
    "Delete"
}
