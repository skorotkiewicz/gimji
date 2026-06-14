use crate::Result;
use crate::models::TabContent;
use crate::storage::Workspace;

use super::{GimjiApp, LoadedTab, SaveStatus};

#[derive(Debug, PartialEq)]
pub(super) enum SelectedContent {
    NoWorkspace,
    NoSelectedTab,
    AlreadyLoaded,
    Loaded { tab_id: String, content: TabContent },
}

pub(super) fn selected_content_for_workspace(
    workspace: Option<&Workspace>,
    current_loaded_tab_id: Option<&str>,
) -> Result<SelectedContent> {
    let Some(workspace) = workspace else {
        return Ok(SelectedContent::NoWorkspace);
    };

    let Some(tab_id) = workspace.selected_tab_id().map(str::to_owned) else {
        return Ok(SelectedContent::NoSelectedTab);
    };

    if current_loaded_tab_id == Some(tab_id.as_str()) {
        return Ok(SelectedContent::AlreadyLoaded);
    }

    let content = workspace.load_tab_content(&tab_id)?;
    Ok(SelectedContent::Loaded { tab_id, content })
}

impl GimjiApp {
    pub(super) fn select_note(&mut self, note_id: String) {
        self.flush_current();
        self.renaming_tab = false;
        self.rename_tab_id = None;
        if let Some(workspace) = &mut self.workspace {
            match workspace.select_note(&note_id) {
                Ok(()) => {
                    self.loaded = None;
                    self.editing_note_title = false;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    pub(super) fn select_tab(&mut self, tab_id: String) {
        self.flush_current();
        self.renaming_tab = false;
        self.rename_tab_id = None;
        if let Some(workspace) = &mut self.workspace {
            match workspace.select_tab(&tab_id) {
                Ok(()) => {
                    self.loaded = None;
                    self.load_selected_content();
                }
                Err(error) => self.set_error(error.to_string()),
            }
        }
    }

    pub(super) fn load_selected_content(&mut self) {
        let current_loaded_tab_id = self.loaded.as_ref().map(|loaded| loaded.tab_id.as_str());

        match selected_content_for_workspace(self.workspace.as_ref(), current_loaded_tab_id) {
            Ok(SelectedContent::NoWorkspace) => {
                self.save_status = SaveStatus::Idle;
            }
            Ok(SelectedContent::NoSelectedTab) => {
                self.loaded = None;
                self.save_status = SaveStatus::Idle;
            }
            Ok(SelectedContent::AlreadyLoaded) => {}
            Ok(SelectedContent::Loaded { tab_id, content }) => {
                self.loaded = Some(LoadedTab {
                    tab_id,
                    content,
                    dirty: false,
                    last_edit: None,
                });
                self.refresh_rename_buffers();
                self.save_status = SaveStatus::Saved;
            }
            Err(error) => self.set_error(error.to_string()),
        }
    }
}
