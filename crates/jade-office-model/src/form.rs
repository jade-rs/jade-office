//! A dialog's controls as data — text boxes, choices, tick boxes, lists,
//! lines of text and buttons — for the dialogs that ask for a lot and do
//! their work in the model: Goal Seek, Data Table, the Scenario Manager,
//! Solver, the Analysis ToolPak, Consolidate, Advanced Filter. Both shells
//! draw a form the same way; what a button does is the dialog's.

use crate::text::TextEdit;

/// Words on a form: an i18n key, text as is (a sheet's or a scenario's
/// name), or a key whose `{name}`s take values.
#[derive(Debug, Clone, PartialEq)]
pub enum Label {
    Key(&'static str),
    Text(String),
    Format(&'static str, Vec<(&'static str, String)>),
}

impl From<&'static str> for Label {
    fn from(key: &'static str) -> Self {
        Label::Key(key)
    }
}

/// One control of a form.
pub enum Control {
    /// A text box: a reference, a value, a name.
    Field { label: Label, edit: TextEdit },
    /// One of several options, shown side by side.
    Choice {
        label: Label,
        options: Vec<Label>,
        chosen: usize,
    },
    /// A tick box.
    Check { label: Label, on: bool },
    /// A list to pick one line of: scenarios, constraints, tools.
    List {
        label: Label,
        items: Vec<String>,
        chosen: Option<usize>,
    },
    /// A line of text: a message, a result.
    Note(Label),
}

/// A control and the name the dialog knows it by.
pub struct Item {
    pub id: &'static str,
    pub control: Control,
}

/// What a key did on a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormKey {
    /// A button, by its id: Enter presses the first, Escape Cancel or Close.
    Press(&'static str),
    Handled,
    Unhandled,
}

/// A dialog's page of controls, with its buttons.
pub struct Form {
    pub title: Label,
    pub items: Vec<Item>,
    /// The buttons along the bottom, the default first.
    pub buttons: Vec<(&'static str, Label)>,
    /// The text box typed into, by its place in `items`.
    pub focus: Option<usize>,
    /// What the last press could not do, as Excel's alert says it.
    pub alert: Option<Label>,
}

impl Form {
    pub fn new(title: impl Into<Label>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
            buttons: Vec::new(),
            focus: None,
            alert: None,
        }
    }

    fn push(mut self, id: &'static str, control: Control) -> Self {
        let field = matches!(control, Control::Field { .. });
        self.items.push(Item { id, control });
        if field && self.focus.is_none() {
            self.focus = Some(self.items.len() - 1);
        }
        self
    }

    /// A text box holding `text`, all of it selected so typing replaces it.
    pub fn field(self, id: &'static str, label: impl Into<Label>, text: &str) -> Self {
        let edit = if text.is_empty() {
            TextEdit::new(String::new())
        } else {
            TextEdit::selected(text.to_string())
        };
        self.push(
            id,
            Control::Field {
                label: label.into(),
                edit,
            },
        )
    }

    pub fn choice(
        self,
        id: &'static str,
        label: impl Into<Label>,
        options: Vec<Label>,
        chosen: usize,
    ) -> Self {
        self.push(
            id,
            Control::Choice {
                label: label.into(),
                options,
                chosen,
            },
        )
    }

    pub fn check(self, id: &'static str, label: impl Into<Label>, on: bool) -> Self {
        self.push(
            id,
            Control::Check {
                label: label.into(),
                on,
            },
        )
    }

    pub fn list(self, id: &'static str, label: impl Into<Label>, items: Vec<String>) -> Self {
        let chosen = (!items.is_empty()).then_some(0);
        self.push(
            id,
            Control::List {
                label: label.into(),
                items,
                chosen,
            },
        )
    }

    pub fn note(self, id: &'static str, label: impl Into<Label>) -> Self {
        self.push(id, Control::Note(label.into()))
    }

    pub fn button(mut self, id: &'static str, label: impl Into<Label>) -> Self {
        self.buttons.push((id, label.into()));
        self
    }

    fn control(&self, id: &str) -> Option<&Control> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .map(|item| &item.control)
    }

    fn control_mut(&mut self, id: &str) -> Option<&mut Control> {
        self.items
            .iter_mut()
            .find(|item| item.id == id)
            .map(|item| &mut item.control)
    }

    /// What the text box `id` holds, trimmed.
    pub fn text(&self, id: &str) -> &str {
        match self.control(id) {
            Some(Control::Field { edit, .. }) => edit.text.trim(),
            _ => "",
        }
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(Control::Field { edit, .. }) = self.control_mut(id) {
            *edit = TextEdit::selected(text.to_string());
        }
    }

    pub fn checked(&self, id: &str) -> bool {
        matches!(self.control(id), Some(Control::Check { on: true, .. }))
    }

    pub fn set_checked(&mut self, id: &str, value: bool) {
        if let Some(Control::Check { on, .. }) = self.control_mut(id) {
            *on = value;
        }
    }

    /// The option chosen of the choice `id`, or the line picked in the list
    /// `id`.
    pub fn chosen(&self, id: &str) -> Option<usize> {
        match self.control(id) {
            Some(Control::Choice { chosen, .. }) => Some(*chosen),
            Some(Control::List { chosen, .. }) => *chosen,
            _ => None,
        }
    }

    pub fn choose(&mut self, id: &str, index: usize) {
        match self.control_mut(id) {
            Some(Control::Choice {
                chosen, options, ..
            }) if index < options.len() => *chosen = index,
            Some(Control::List { chosen, items, .. }) => {
                *chosen = (index < items.len()).then_some(index)
            }
            _ => {}
        }
    }

    /// The list `id`'s lines anew, the one picked kept where it still is.
    pub fn set_list(&mut self, id: &str, lines: Vec<String>) {
        if let Some(Control::List { items, chosen, .. }) = self.control_mut(id) {
            *chosen = match *chosen {
                Some(at) if at < lines.len() => Some(at),
                _ if lines.is_empty() => None,
                _ => Some(lines.len() - 1),
            };
            *items = lines;
        }
    }

    pub fn set_note(&mut self, id: &str, label: Label) {
        if let Some(Control::Note(note)) = self.control_mut(id) {
            *note = label;
        }
    }

    /// The text box typed into.
    pub fn typing(&self) -> Option<&TextEdit> {
        match &self.items.get(self.focus?)?.control {
            Control::Field { edit, .. } => Some(edit),
            _ => None,
        }
    }

    pub fn typing_mut(&mut self) -> Option<&mut TextEdit> {
        let at = self.focus?;
        match &mut self.items.get_mut(at)?.control {
            Control::Field { edit, .. } => Some(edit),
            _ => None,
        }
    }

    /// A click on the control at `index`: a text box takes the typing, a
    /// tick box turns over, a choice or a list takes `option`. Returns the
    /// control's id when something changed that the dialog may follow.
    pub fn click(&mut self, index: usize, option: usize) -> Option<&'static str> {
        let item = self.items.get_mut(index)?;
        match &mut item.control {
            Control::Field { .. } => {
                self.focus = Some(index);
                None
            }
            Control::Check { on, .. } => {
                *on = !*on;
                Some(item.id)
            }
            Control::Choice {
                options, chosen, ..
            } if option < options.len() => {
                *chosen = option;
                Some(item.id)
            }
            Control::List { items, chosen, .. } if option < items.len() => {
                *chosen = Some(option);
                Some(item.id)
            }
            _ => None,
        }
    }

    /// The form's keys: Enter presses the default button, Escape Cancel (or
    /// Close), Tab goes to the next text box, the rest edit the one typed
    /// into.
    pub fn key(&mut self, key: &str, command: bool) -> FormKey {
        if self.typing().is_some_and(|edit| edit.marked.is_some()) {
            return FormKey::Unhandled;
        }
        match key {
            "enter" => self
                .buttons
                .first()
                .map_or(FormKey::Handled, |(id, _)| FormKey::Press(id)),
            "escape" => self
                .buttons
                .iter()
                .find(|(id, _)| matches!(*id, "cancel" | "close"))
                .map_or(FormKey::Handled, |(id, _)| FormKey::Press(id)),
            "tab" => {
                let fields: Vec<usize> = (0..self.items.len())
                    .filter(|&at| matches!(self.items[at].control, Control::Field { .. }))
                    .collect();
                if let Some(&first) = fields.first() {
                    self.focus = Some(
                        fields
                            .iter()
                            .copied()
                            .find(|&at| Some(at) > self.focus)
                            .unwrap_or(first),
                    );
                }
                FormKey::Handled
            }
            "a" if command => {
                if let Some(edit) = self.typing_mut() {
                    edit.whole = true;
                }
                FormKey::Handled
            }
            key => match self.typing_mut() {
                Some(edit) if edit.edit_key(key) => FormKey::Handled,
                _ => FormKey::Unhandled,
            },
        }
    }
}
