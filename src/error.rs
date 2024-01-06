use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq)]
pub enum CtGenError {
    InitError(String),
    ValidationError(String),
    RuntimeError(String),
}

impl Display for CtGenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CtGenError::InitError(s) => {
                write!(f, "InitError: {}", s)
            }
            CtGenError::ValidationError(s) => {
                write!(f, "ValidationError: {}", s)
            }
            CtGenError::RuntimeError(s) => {
                write!(f, "RuntimeError: {}", s)
            }
        }
    }
}

impl std::error::Error for CtGenError {}