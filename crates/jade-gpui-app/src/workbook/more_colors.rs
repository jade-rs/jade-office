use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::more_colors::{MoreColorsDialog, MoreColorsKey};
type View = WorkbookView;
include!("more_colors_ui.rs");
impl WorkbookView {
    fn more_colors_editor(&self) -> Option<&MoreColorsDialog> {
        self.more_colors.as_ref()
    }
    /// More Colors… from the fill (true) or font colour gallery.
    pub(super) fn open_more_colors(&mut self, fill: bool, cx: &mut Context<Self>) {
        let look = self.book.format_at(self.active());
        let current = if fill {
            look.fill_color
        } else {
            look.font_color
        };
        self.popup = None;
        self.more_colors = Some(MoreColorsDialog::new(fill, current.as_deref()));
        cx.notify();
    }
    pub(super) fn more_colors_action(&mut self, action: MoreColorsAction, cx: &mut Context<Self>) {
        match action {
            MoreColorsAction::Take => {
                // A code that is not a colour keeps the dialog open.
                let Some(color) = self.more_colors.as_ref().and_then(|dialog| dialog.color())
                else {
                    return;
                };
                let Some(dialog) = self.more_colors.take() else {
                    return;
                };
                let range = self.range();
                let result = if dialog.fill {
                    self.book.fill_color(range, Some(&color))
                } else {
                    self.book.font_color(range, &color)
                };
                self.after_write(result, cx);
            }
            MoreColorsAction::Cancel => {
                self.more_colors = None;
                cx.notify();
            }
        }
    }
    pub(super) fn more_colors_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self.more_colors.as_mut() else {
            return false;
        };
        match dialog.key(key, command) {
            MoreColorsKey::Take => self.more_colors_action(MoreColorsAction::Take, cx),
            MoreColorsKey::Close => self.more_colors_action(MoreColorsAction::Cancel, cx),
            MoreColorsKey::Handled => cx.notify(),
            MoreColorsKey::Unhandled => return false,
        }
        true
    }
}
