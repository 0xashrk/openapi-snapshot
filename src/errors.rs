#[derive(Debug)]
pub enum AppError {
    Usage(String),
    Network(String),
    Json(String),
    Reduce(String),
    Outline(String),
    Io(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Usage(_) => 1,
            AppError::Network(_) => 1,
            AppError::Json(_) => 2,
            AppError::Reduce(_) => 3,
            AppError::Outline(_) => 3,
            AppError::Io(_) => 4,
        }
    }

    pub fn is_url_related(&self) -> bool {
        matches!(self, AppError::Network(_) | AppError::Json(_))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Usage(msg)
            | AppError::Network(msg)
            | AppError::Json(msg)
            | AppError::Reduce(msg)
            | AppError::Outline(msg)
            | AppError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AppError {}
