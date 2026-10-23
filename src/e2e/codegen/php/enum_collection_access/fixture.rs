pub struct Reply {
    pub choices: Vec<Choice>,
}
pub struct Choice {
    pub message: Message,
}
pub struct Message {
    pub content: Option<Body>,
}
#[serde(untagged)]
pub enum Body {
    Text(String),
    Parts(Vec<Piece>),
}
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Piece {
    Text { text: String },
    Note { note: String },
}
