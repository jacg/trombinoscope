use std::path::Path;

use crate::{
    FaceType,
    error::Result,
};

/// Interface for manipulating and displaying a cropped and labelled face in the UI.
pub trait Face {
    /// UI backend-specific information needed to dislpay the face
    type View;
    fn load(path: impl AsRef<Path>) -> Result<FaceType<Self>> where Self: Sized;
    fn replace_image(&mut self, path: impl AsRef<Path>) -> Result<()>;
    fn set_cx (&mut self, x: f32)                       -> Result<f32>;
    fn set_cy (&mut self, y: f32)                       -> Result<f32>;
    fn set_w  (&mut self, w: f32)                       -> Result<f32>;
    fn set_rot(&mut self, rot: i8)                      -> Result<i8>;
    fn save(&self)                                      -> Result<()>;
    fn view(&self, face: &FaceType<Self>, view: &Self::View) -> Result<()> where Self: Sized;
    fn full_w(&self) -> Result<f32>;
    fn full_h(&self) -> Result<f32>;
}
