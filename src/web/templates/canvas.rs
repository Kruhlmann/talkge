use askama::Template;

#[derive(Debug, Template)]
#[template(path = "canvas.html")]
pub struct CanvasTemplate {
    pub token: String,
}
