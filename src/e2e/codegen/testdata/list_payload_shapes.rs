#[derive(serde::Serialize)]
pub struct Reply {
    pub summary: String,
    pub content: Option<Body>,
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum Body {
    Text(String),
    Parts(Vec<Piece>),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Piece {
    Group {
        #[serde(rename = "values")]
        items: Vec<String>,
    },
    Text {
        #[serde(rename = "value")]
        text: String,
    },
}

pub fn reply() -> Reply {
    Reply {
        summary: "report".into(),
        content: Some(Body::Parts(vec![
            Piece::Text { text: "hello".into() },
            Piece::Text { text: " world".into() },
        ])),
    }
}
