use std::path::Path;

use face::{
    FaceInImage,
    error as ferr,
};
use image::{DynamicImage, GenericImageView};
use show_image::WindowProxy;

pub (crate) type FaceSi<'a> = face::Face<CropSi<'a>>;

pub (crate) struct CropSi<'w> {
    pub (crate) rot: i8,
    pub (crate) rotated_image: DynamicImage,
    pub (crate) window: &'w WindowProxy,
}

#[derive(Clone, Copy)]
pub (crate) struct ViewSi {}

impl face::CropUi for CropSi<'_> {
    type View = ViewSi;

    fn replace_image(&mut self, path: impl AsRef<Path>) -> ferr::Result<()> {
        todo!()
    }

    fn set_cx (&mut self, x: f32) -> ferr::Result<f32> { Ok(x) }
    fn set_cy (&mut self, y: f32) -> ferr::Result<f32> { Ok(y) }
    fn set_w  (&mut self, w: f32) -> ferr::Result<f32> { Ok(w) }
    fn set_rot(&mut self, new_rot: i8) -> ferr::Result<i8> {
        let old_rot = self.rot;
        let old_rot_img = &self.rotated_image;
        self.rotated_image = match (new_rot - old_rot).rem_euclid(4) {
            0 => old_rot_img.clone(),
            1 => old_rot_img.rotate90(),
            2 => old_rot_img.rotate180(),
            3 => old_rot_img.rotate270(),
            _ => unreachable!(),
        };
        self.rot = new_rot;
        Ok(new_rot)
    }

    fn save(&self) -> ferr::Result<()> { todo!() }

    fn view(
        &self,
        face: &FaceInImage,
        _: &Self::View
    ) -> ferr::Result<()> {
        let &FaceInImage { cx, cy, w, ..  } = face;
        let x = (cx - w / 2.0) as u32;
        let y = (cy - w / 2.0) as u32;
        let width = w as u32;
        let height = w as u32 * 5 / 4; // TODO replace magic number with ASPECT_RATIO
        let cropped = self.rotated_image.crop_imm(x, y, width, height);
        self.window.set_image("TODO label", cropped);
        Ok(())
    }

    fn full_w(&self) -> ferr::Result<f32> { Ok(self.rotated_image.dimensions().0 as f32) }
    fn full_h(&self) -> ferr::Result<f32> { Ok(self.rotated_image.dimensions().1 as f32) }
}
