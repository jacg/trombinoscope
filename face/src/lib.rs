//! TODO LIES, DAMNED LIES AND COMMENTS THAT DIVERGE FROM IMPLEMENTATION
//!
//! Abstractions for the identification and display of faces found in JPEG
//! images, reusable across different UI backends.
//!
//! The four major components are:
//!
//! + `struct FaceInImage`
//!
//!   The location, orientation and label of a face within an image
//!
//! + `trait FaceUiTrait`
//!
//!   Data and functionality needed to manipulate `FaceInImage` in any given UI
//!   backend.
//!
//! + `struct FaceType<Ui: FaceUiTrait>`
//!
//!   Representation and manipulation of a single face in a specific UI backend.
//!
//!   - path to the full image containing a face
//!   - `FaceInImage` describing where to find the face within the full image
//!   - `FaceUiTrait` instance for the UI backend being used.
//!
//!   + TODO add trait `UiGlobal` ?

mod metadata;
pub use metadata::FaceInImage;

pub mod ui;

pub mod error;
pub mod old;

use std::path::{Path, PathBuf};

use crate::error::Result;

/// A named face in a photograph.
///
/// + `path`: location of the full JPEG containing the face
///
/// + `face`: stores the location and orientation of the face in the full image,
///   as well as its label (name and surname). This information is cached in the
///   metadata of the JPEG at `path`.
///
/// + `ui`: user interface for cropping and labelling a face contained in the
///    JPEG. Its type must implement the `Ui` trait for any UI backend.
#[derive(Debug)]
pub struct FaceType<Ui: ui::one::Face> {
    pub path: PathBuf,
    pub face: FaceInImage,
    pub ui: Ui,
}

macro_rules! meth_coordinated_with_face {
    ($outer_meth:ident $inner_meth:ident $attr:ident $type:ty) => {
        pub fn $outer_meth(&mut self, delta: $type) -> Result<$type> {
            self.
                ui
                .$inner_meth(self.face.$attr - delta)
                .inspect(|&res| self.face.$attr = res)
        }
    };
}

impl<Ui: ui::one::Face> FaceType<Ui> {

    pub fn load(path: impl AsRef<Path>) -> Result<Self> { Ui::load(path) }

    meth_coordinated_with_face!{move_right set_cx  cx  f32}
    meth_coordinated_with_face!{move_down  set_cy  cy  f32}
    meth_coordinated_with_face!{zoom_in    set_w   w   f32}
    meth_coordinated_with_face!{rotate     set_rot rot i8 }

    pub fn view(&self, view: &Ui::View) -> Result<()> {
        self.ui.view(self, view)
    }
}

pub const ASPECT_RATIO: f32 = 5.0 / 4.0;
