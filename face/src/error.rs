use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error{

    #[error("No face metadata was found in {0}")]
    NoMetadataFound(PathBuf),

    #[error("Something went wrong in our use of `bitcode`. Old metadata version? If so, strip")]
    Bitcode(#[from] bitcode::Error),

    #[error("This has not been implemented yet")]
    Todo,
}

pub type Result<T> = std::result::Result<T, Error>;
