//! Identifies the current rich copy without confusing equal text from elsewhere.

use jade_web::word::clipboard::RichClip;

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

#[derive(Clone)]
pub(crate) struct RichCopy {
    pub clip: RichClip,
    marker: String,
    html: String,
}

impl RichCopy {
    pub fn new(clip: RichClip) -> Self {
        // An identity hint, not a security boundary. Distinguish copies across
        // browser instances as well as consecutive copies within this instance.
        let marker = format!("{}-{}", js_sys::Date::now(), js_sys::Math::random());
        let html =
            clip.to_html()
                .replacen("<p ", &format!("<p data-jade-clipboard=\"{marker}\" "), 1);
        Self { clip, marker, html }
    }

    pub fn html(&self) -> &str {
        &self.html
    }

    pub fn matches(&self, text: &str, html: Option<&str>) -> bool {
        let Some(html) = html else { return false };
        let marker_present = ['"', '\''].into_iter().any(|quote| {
            html.contains(&format!(
                "data-jade-clipboard={quote}{}{quote}",
                self.marker
            ))
        });
        // An external editor may retain the identity attribute while changing
        // the formatting. Compare the exported representation as well, retaining
        // richer in-process properties only for an unchanged copy.
        self.clip.matches(text)
            && marker_present
            && RichClip::from_html(html) == RichClip::from_html(&self.html)
    }
}
