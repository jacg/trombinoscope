//! Abstractions for the identification and display of faces found in JPEG
//! images, reusable across different UI backends.
//!
//! The three major components are:
//!
//! + `struct FaceInImage`
//!
//!   The location, orientation and label of a face within an image
//!
//! + `trait CropUi`
//!
//!   Data and functionality needed to manipulate `FaceInImage` in any given UI
//!   backend.
//!
//! + `struct Face<C: CropUi>`
//!
//!   Representation and manipulation of a single face in a specific UI backend.
//!
//!   - path to the full image containing a face
//!   - `FaceInImage` describing where to find the face within the full image
//!   - `CropUi` instance for the UI backend being used.

mod metadata;
pub use metadata::FaceInImage;

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
/// + `crop`: user interface for cropping and labelling a face contained in the
///    JPEG. Its type must implement the `Crop` trait for any UI backend.
pub struct Face<Crop: CropUi> {
    path: PathBuf,
    face: FaceInImage,
    crop: Crop,
}

/// Interface for manipulating and displaying a cropped and labelled face in the UI.
pub trait CropUi {
    /// UI backend-specific information needed to dislpay the face
    type View;
    fn replace_image(&mut self, path: impl AsRef<Path>) -> Result<()>;
    fn set_cx (&mut self, x: f32)                       -> Result<f32>;
    fn set_cy (&mut self, y: f32)                       -> Result<f32>;
    fn set_w  (&mut self, w: f32)                       -> Result<f32>;
    fn set_rot(&mut self, rot: i8)                      -> Result<i8>;
    fn save(&self)                                      -> Result<()>;
    fn view(
        &self,
        face: &FaceInImage,
        view: &Self::View
    ) -> Result<()>;
    fn full_w(&self) -> Result<f32>;
    fn full_h(&self) -> Result<f32>;
}

macro_rules! meth_coordinated_with_face {
    ($outer_meth:ident $inner_meth:ident $attr:ident) => {
        pub fn $outer_meth(&mut self, delta: f32) -> Result<f32> {
            self.
                crop
                .$inner_meth(self.face.$attr - delta)
                .inspect(|&res| self.face.$attr = res)
        }
    };
}

impl<Crop: CropUi> Face<Crop> {

    /// Create face at default location in image, ignoring any metadata that might be present
    pub fn new(path: impl AsRef<Path>, crop: Crop) -> Result<Self> {

        let i = &crop;
        let (full_w, full_h, rot) = {
            let x = i.full_w().unwrap();
            let y = i.full_h().unwrap();
            if x < y {(x, y, 0)} else {(y, x, 3)}
        };

        let (frac_cx, frac_cy, frac_w) = (0.5, 0.15, 0.18);
        let w  = frac_w *  full_w;
        let cx = frac_cx * full_w;
        let cy = frac_cy * full_h;

        let basename = path.as_ref().file_name().unwrap();
        let (given, family) = util::filename_to_given_family(basename).unwrap();
        let face = FaceInImage {
            given, family,
            cx, cy, w,
            rot,
        };
        let new = Self {
            path: path.as_ref().to_owned(),
            face,
            crop,
        };
        Ok(new)
    }

    /// Create face using metadata found in image. If no metadata is present,
    /// use default metadata.
    pub fn from_metadata(path: impl AsRef<Path>) -> Result<Self> {
        // let mut new = Self::new(&path, image);
        // let mut jpeg = read_jpeg(&path);
        // if strip_old_metadata { jpeg.remove_segments_by_marker(OUR_MARKER) }
        Ok(todo!())
    }

    /// NOT IMPLEMENTED YET: look for face metadata in JPEG at `path`
    pub fn find_in_path(path: impl AsRef<Path>) -> Result<Self> { todo!() }

    /// Remove all of our metadata from the JPEG image at `path`
    pub fn strip_metadata(path: impl AsRef<Path>) -> Result<()> { todo!() }

    meth_coordinated_with_face!{move_right set_cx cx}
    meth_coordinated_with_face!{move_down  set_cy cy}
    meth_coordinated_with_face!{zoom_in    set_w  w }

    pub fn rotate(&mut self, rot: i8) -> Result<i8> {
        let rot = (self.face.rot + rot).rem_euclid(4);
        self
            .crop
            .set_rot(rot)
            .inspect(|_| self.face.rot = rot)
    }

    pub fn view(&self, view: Crop::View) -> Result<()> {
        self.crop.view(&self.face, &view)
    }
}
