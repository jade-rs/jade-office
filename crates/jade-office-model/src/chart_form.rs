//! A dialog of labelled fields — a line of text, a choice of options, a
//! tick box — as the chart dialogs ask with them. Both shells draw one the
//! same way: a row a field, Tab from one text to the next, Enter for OK
//! and Escape for Cancel.

use crate::text::TextEdit;

/// What a field holds.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Text(TextEdit),
    /// One of `options` (i18n keys), the `chosen` one.
    Choice {
        options: Vec<&'static str>,
        chosen: usize,
    },
    Check(bool),
}

/// One row of a form: its words (an i18n key, which also names it) and
/// what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub key: &'static str,
    pub value: FieldValue,
}

/// What a key did to a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    /// The dialog's title, an i18n key.
    pub title: &'static str,
    pub fields: Vec<Field>,
    /// The field typing goes into, when it is a text.
    pub focus: usize,
}

impl Form {
    pub fn new(title: &'static str) -> Self {
        Self {
            title,
            fields: Vec::new(),
            focus: usize::MAX,
        }
    }

    /// A text more, `value` in it; the first text takes the typing.
    pub fn text(mut self, key: &'static str, value: impl Into<String>) -> Self {
        if self.focus == usize::MAX {
            self.focus = self.fields.len();
        }
        self.fields.push(Field {
            key,
            value: FieldValue::Text(TextEdit::selected(value.into())),
        });
        self
    }

    /// A choice more, of `options`, `chosen` picked.
    pub fn choice(mut self, key: &'static str, options: &[&'static str], chosen: usize) -> Self {
        self.fields.push(Field {
            key,
            value: FieldValue::Choice {
                options: options.to_vec(),
                chosen: chosen.min(options.len().saturating_sub(1)),
            },
        });
        self
    }

    /// A tick box more.
    pub fn check(mut self, key: &'static str, on: bool) -> Self {
        self.fields.push(Field {
            key,
            value: FieldValue::Check(on),
        });
        self
    }

    fn field(&self, key: &str) -> Option<&FieldValue> {
        self.fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| &field.value)
    }

    /// What is typed in the text `key`; empty when there is none.
    pub fn text_of(&self, key: &str) -> &str {
        match self.field(key) {
            Some(FieldValue::Text(text)) => text.text.trim(),
            _ => "",
        }
    }

    /// Which option of the choice `key` is picked.
    pub fn chosen(&self, key: &str) -> usize {
        match self.field(key) {
            Some(FieldValue::Choice { chosen, .. }) => *chosen,
            _ => 0,
        }
    }

    /// Whether the tick box `key` is ticked.
    pub fn checked(&self, key: &str) -> bool {
        matches!(self.field(key), Some(FieldValue::Check(true)))
    }

    /// Whether the form has the field `key`.
    pub fn has(&self, key: &str) -> bool {
        self.field(key).is_some()
    }

    /// The text typing goes into.
    pub fn typing(&mut self) -> Option<&mut TextEdit> {
        match self
            .fields
            .get_mut(self.focus)
            .map(|field| &mut field.value)
        {
            Some(FieldValue::Text(text)) => Some(text),
            _ => None,
        }
    }

    pub fn typed(&self) -> Option<&TextEdit> {
        match self.fields.get(self.focus).map(|field| &field.value) {
            Some(FieldValue::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// A click on the field at `index`: a text takes the typing, a tick
    /// box flips.
    pub fn click(&mut self, index: usize) {
        match self.fields.get_mut(index).map(|field| &mut field.value) {
            Some(FieldValue::Text(_)) => self.focus = index,
            Some(FieldValue::Check(on)) => *on = !*on,
            _ => {}
        }
    }

    /// The choice at `index` set to its `option`.
    pub fn choose(&mut self, index: usize, option: usize) {
        if let Some(FieldValue::Choice { options, chosen }) =
            self.fields.get_mut(index).map(|field| &mut field.value)
        {
            if option < options.len() {
                *chosen = option;
            }
        }
    }

    /// Enter takes it, Escape leaves it, Tab goes to the next text; the
    /// rest edit the text typed into.
    pub fn key(&mut self, key: &str, command: bool) -> FormKey {
        if self.typed().is_some_and(|text| text.marked.is_some()) {
            return FormKey::Unhandled;
        }
        match key {
            "enter" => return FormKey::Take,
            "escape" => return FormKey::Close,
            "tab" => {
                let texts: Vec<usize> = (0..self.fields.len())
                    .filter(|&at| matches!(self.fields[at].value, FieldValue::Text(_)))
                    .collect();
                if let Some(next) = texts.iter().find(|&&at| at > self.focus).or(texts.first()) {
                    self.focus = *next;
                }
                return FormKey::Handled;
            }
            _ => {}
        }
        let Some(text) = self.typing() else {
            return FormKey::Unhandled;
        };
        if key == "a" && command {
            text.whole = true;
            return FormKey::Handled;
        }
        if text.edit_key(key) {
            FormKey::Handled
        } else {
            FormKey::Unhandled
        }
    }
}
