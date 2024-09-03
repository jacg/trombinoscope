use std::path::{Path, PathBuf};

use crate::error::Result;

pub struct Face<Image: InMemoryImage> {
    full_image_path: PathBuf,
    detail: FaceInImage,
    in_memory_image: Image,
}

pub trait InMemoryImage {
    type Render; // UI-specific information needed by render
    fn replace_image(&mut self, path: impl AsRef<Path>) -> Result<()>;
    fn move_right  (&mut self, dx: f32)                 -> Result<f32>;
    fn move_down   (&mut self, dy: f32)                 -> Result<f32>;
    fn change_width(&mut self, dw: f32)                 -> Result<f32>;
    fn rotate(&mut self, rot: i8)                       -> Result<i8>;
    fn save(&self)                                      -> Result<()>;
    fn render(
        &self,
        detail: &FaceInImage,
        x: &Self::Render
    ) -> Result<()>;
}

impl<C: InMemoryImage> Face<C> {
    pub fn new_from_path(path: impl AsRef<Path>) -> Result<Self> { todo!() }
    pub fn find_in_path(path: impl AsRef<Path>) -> Result<Self> { todo!() }

    /// Remove all of our metadata from the JPEG image at `path`
    pub fn strip_metadata(path: impl AsRef<Path>) -> Result<()> { todo!() }

    pub fn move_right(&mut self, dx: f32) -> Result<f32> {
        self.in_memory_image.move_right(dx).inspect(|&x| self.detail.cx = x)
    }

    pub fn move_down(&mut self, dy: f32) -> Result<f32> {
        self.in_memory_image.move_down(dy).inspect(|&y| self.detail.cy = y)
    }

    pub fn change_width(&mut self, dw: f32) -> Result<f32> {
        self.in_memory_image.change_width(dw).inspect(|&w| self.detail.w = w)
    }

    pub fn rotate(&mut self, rot: i8) -> Result<i8> {
        self
            .in_memory_image
            .rotate(rot)
            .inspect(|&qtc| self.detail.rot = qtc)
    }
}

use bitcode::{Decode, Encode};

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct FaceInImage {
    pub given: String,
    pub family: String,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub rot: i8,
}
