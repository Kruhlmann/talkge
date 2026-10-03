use askama::Template;

use crate::web::{templates::CommonTemplateData, SessionUser};

#[derive(Template, Debug)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    common: CommonTemplateData,
    user: Option<SessionUser>,
    canvas_url: Option<String>,
}

impl IndexTemplate {
    pub fn new(user: Option<SessionUser>, canvas_url: Option<String>) -> Self {
        Self {
            common: CommonTemplateData::default(),
            canvas_url,
            user,
        }
    }
}
