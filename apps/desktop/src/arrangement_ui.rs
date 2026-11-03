use crate::app::*;
use garrido_studio::arrangement::{self, ClipId, Tool};

impl Studio {
    pub(crate) fn arrangement_action(&mut self, tag: isize) -> bool {
        if self.page != Page::Arrange || !matches!(tag, 440..=444 | 9060..=9066) {
            return false;
        }
        let result = (|| -> Result<()> {
            if tag == 440 {
                self.workspace.selected_clips = arrangement::all(&self.history.project).collect();
                return Ok(());
            }
            let selection = &self.workspace.selected_clips;
            if tag == 441 || tag == 442 {
                self.workspace.clip_clipboard =
                    Some(arrangement::copy(&self.history.project, selection)?);
            }
            if tag == 441 {
                return Ok(());
            }
            let next = if tag == 443 {
                arrangement::paste(
                    &self.history.project,
                    self.workspace
                        .clip_clipboard
                        .as_ref()
                        .ok_or(Error::Invalid("No hay clips copiados"))?,
                    self.workspace.arrangement_cursor,
                )?
            } else {
                let tool = match tag {
                    442 | 444 => Tool::Delete,
                    9060 => Tool::Split(self.workspace.arrangement_cursor),
                    9061 => Tool::Join,
                    9062 => Tool::Mute,
                    9063 => Tool::Group,
                    9064 => Tool::Ungroup,
                    9066 => Tool::TrimStart(self.workspace.arrangement_cursor),
                    _ => Tool::Duplicate(self.workspace.arrangement_cursor),
                };
                arrangement::apply(&self.history.project, selection, tool)?
            };
            let label = match tag {
                442 => "Cut clips",
                443 => "Paste clips",
                444 => "Delete clips",
                9060 => "Split clips",
                9061 => "Join clips",
                9062 => "Mute clips",
                9063 => "Group clips",
                9064 => "Ungroup clips",
                9066 => "Trim clips",
                _ => "Duplicate clips",
            };
            self.history.edit_named(label, |p| {
                *p = next;
                Ok(())
            })?;
            if matches!(tag, 442..=444 | 9060 | 9061 | 9065) {
                self.workspace.selected_clips.clear();
            }
            self.changed();
            Ok(())
        })();
        self.result(result);
        self.redraw();
        true
    }
    pub(crate) fn select_clip(&mut self, id: ClipId, additive: bool) {
        if additive && self.workspace.selected_clips.contains(&id) {
            let group =
                arrangement::members(&self.history.project, &[id]).unwrap_or_else(|_| vec![id]);
            self.workspace.selected_clips.retain(|c| !group.contains(c));
        } else if additive || !self.workspace.selected_clips.contains(&id) {
            if !additive {
                self.workspace.selected_clips.clear();
            }
            self.workspace.selected_clips.push(id);
            self.workspace.selected_clips =
                arrangement::members(&self.history.project, &self.workspace.selected_clips)
                    .unwrap_or_default();
        }
    }
}
