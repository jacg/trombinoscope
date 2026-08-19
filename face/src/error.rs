use std::{ffi::OsString, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error{

    #[error("No face metadata was found in {0}")]
    NoMetadataFound(String),

    #[error("Something went wrong in our use of `bitcode`. Old metadata version? If so, strip")]
    Bitcode(#[from] bitcode::Error),

    #[error("Could not find basename in {0}")]
    NoBaseName(PathBuf),

    #[error("TODO OsString error description")]
    Abcd(OsString),

    #[error("Face not loaded yet")]
    FaceNotLoaded,

    #[error("TODO io::Error description")]
    XXX(#[from] std::io::Error),

    #[error(transparent)]
    Util(#[from] util::Error),

    // TODO: this is probably too backend-specific to appear here
    #[error("Something went wrong in our use of `image`")]
    Image(#[from] image::ImageError),

    #[error("This has not been implemented yet")]
    Todo,
}

pub type Result<T> = std::result::Result<T, Error>;
