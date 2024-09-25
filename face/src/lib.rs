pub mod metadata;
pub mod error;

pub use metadata::FaceInImage;
pub use error::{Error, Result};

pub const ASPECT_RATIO: f32 = 5.0 / 4.0;
