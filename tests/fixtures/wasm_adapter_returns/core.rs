#[derive(Clone)]
pub struct Client;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Reply {
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("request rejected: {message}")]
    Rejected { message: String, status: u16 },
    #[error("service unavailable")]
    Unavailable,
}

pub fn reply_sync() -> Result<Reply, ApiError> {
    Ok(Reply {
        message: "ready".into(),
    })
}

impl Client {
    pub fn sync_reply(&self) -> Result<Reply, ApiError> {
        reply_sync()
    }
    pub async fn reply(&self) -> Result<Reply, ApiError> {
        reply_sync()
    }
    pub async fn optional_reply(&self) -> Result<Option<Reply>, ApiError> {
        reply_sync().map(Some)
    }
    pub async fn replies(&self) -> Result<Vec<Reply>, ApiError> {
        reply_sync().map(|reply| vec![reply])
    }
    pub async fn optional_replies(&self) -> Result<Option<Vec<Reply>>, ApiError> {
        self.replies().await.map(Some)
    }
    pub async fn sparse_replies(&self) -> Result<Vec<Option<Reply>>, ApiError> {
        self.optional_reply().await.map(|reply| vec![reply])
    }
    pub async fn flush(&self) -> Result<(), ApiError> {
        Ok(())
    }
    pub async fn bytes(&self) -> Result<bytes::Bytes, ApiError> {
        Ok(bytes::Bytes::from_static(b"ready"))
    }
}
