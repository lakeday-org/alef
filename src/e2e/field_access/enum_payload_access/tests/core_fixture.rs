#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum Payload {
    Segments(Vec<Piece>),
    Text(String),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum Piece {
    Text {
        #[serde(rename = "value")]
        text: String,
    },
    Note {
        note_text: String,
    },
}

#[derive(serde::Serialize)]
pub enum ExternalPayload {
    #[serde(rename = "segmentsWire")]
    Segments(Vec<Piece>),
}

#[derive(serde::Serialize)]
#[serde(tag = "tag", content = "body")]
pub enum AdjacentPayload {
    Segments(Vec<Piece>),
}

#[derive(serde::Serialize)]
pub enum ExternalPiece {
    Text { text: String },
}

#[derive(serde::Serialize)]
pub enum ExternalPieces {
    Segments(Vec<ExternalPiece>),
}

#[derive(serde::Serialize)]
pub enum AmbiguousPiece {
    First { shared: String },
    Second { shared: String },
}

#[derive(serde::Serialize)]
pub enum AmbiguousPayload {
    Segments(Vec<AmbiguousPiece>),
}
