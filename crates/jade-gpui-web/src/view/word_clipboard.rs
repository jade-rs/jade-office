//! Identifies the current rich copy without confusing equal text from elsewhere.

#[derive(Clone)]
pub(crate) struct ObjectCopy {
    pub clip: jade_web::word::objects::ObjectClip,
    html: String,
}

impl ObjectCopy {
    pub fn new(clip: jade_web::word::objects::ObjectClip) -> Self {
        Self {
            clip,
            html: format!(
                "<span data-jade-object=\"{}-{}\">&#xfffc;</span>",
                js_sys::Date::now(),
                js_sys::Math::random()
            ),
        }
    }

    pub fn html(&self) -> &str {
        &self.html
    }

    pub fn matches(&self, text: &str, html: Option<&str>) -> bool {
        text == "\u{fffc}"
            && html.is_some_and(|html| {
                html.contains(&self.html)
                    || html.contains(&self.html.replace("&#xfffc;", "\u{fffc}"))
            })
    }
}

pub(crate) use jade_web::word::clipboard::RichClipboardCopy as RichCopy;
