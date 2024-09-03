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
    fn set_cx (&mut self, x: f32)                       -> Result<f32>;
    fn set_cy (&mut self, y: f32)                       -> Result<f32>;
    fn set_w  (&mut self, w: f32)                       -> Result<f32>;
    fn set_rot(&mut self, rot: i8)                      -> Result<i8>;
    fn save(&self)                                      -> Result<()>;
    fn render(
        &self,
        detail: &FaceInImage,
        x: &Self::Render
    ) -> Result<()>;
    fn full_w(&self) -> Result<f32>;
    fn full_h(&self) -> Result<f32>;
}

macro_rules! meth_coordinated_with_details {
    ($outer_meth:ident $inner_meth:ident $attr:ident) => {
        pub fn $outer_meth(&mut self, delta: f32) -> Result<f32> {
            self.
                in_memory_image
                .$inner_meth(self.detail.$attr - delta)
                .inspect(|&res| self.detail.$attr = res)
        }
    };
}

impl<MemImg: InMemoryImage> Face<MemImg> {
    pub fn new(path: impl AsRef<Path>, in_memory_image: MemImg) -> Result<Self> {

        let i = &in_memory_image;
        let (full_w, full_h, rot) = {
            let x = i.full_w().unwrap();
            let y = i.full_h().unwrap();
            if x < y {(x, y, 0)} else {(y, x, 3)}
        };

        let (frac_cx, frac_cy, frac_w) = (0.5, 0.15, 0.18);
        let w  = frac_w *  full_w;
        let cx = frac_cx * full_w;
        let cy = frac_cy * full_h;

        let detail = FaceInImage {
            given: "TODO Prénom".into(),
            family:"TODO Nom".into(),
            cx, cy, w,
            rot,
        };
        let new = Self {
            full_image_path: path.as_ref().to_owned(),
            detail,
            in_memory_image,
        };
        Ok(new)
    }
    pub fn find_in_path(path: impl AsRef<Path>) -> Result<Self> { todo!() }

    /// Remove all of our metadata from the JPEG image at `path`
    pub fn strip_metadata(path: impl AsRef<Path>) -> Result<()> { todo!() }

    meth_coordinated_with_details!{move_right set_cx cx}
    meth_coordinated_with_details!{move_down  set_cy cy}
    meth_coordinated_with_details!{enlarge    set_w  w }

    pub fn rotate(&mut self, rot: i8) -> Result<i8> {
        let rot = (self.detail.rot + rot).rem_euclid(4);
        self
            .in_memory_image
            .set_rot(rot)
            .inspect(|_| self.detail.rot = rot)
    }

    pub fn render(&self, render: MemImg::Render) -> Result<()> {
        self.in_memory_image.render(&self.detail, &render)
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
