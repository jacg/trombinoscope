pub mod metadata;
pub mod ui;
pub mod error;

pub use metadata::FaceInImage;
pub use error::{Error, Result};

use std::path::PathBuf;

#[derive(Debug)]
pub struct FaceType<Ui: ui::one::Face> {
    pub path: PathBuf,
    pub face: FaceInImage,
    pub ui: Ui,
}

impl<Ui: ui::one::Face> FaceType<Ui> {

    pub fn save_metadata(&self) -> Result<()> { self.face.embed_in_jpeg(&self.path) }


}

pub const ASPECT_RATIO: f32 = 5.0 / 4.0;
