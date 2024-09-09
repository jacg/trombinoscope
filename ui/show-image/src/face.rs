use std::path::Path;

use face::{
    FaceInImage,
    error as ferr,
};
use image::{DynamicImage, GenericImageView};
use show_image::WindowProxy;

pub (crate) type SiFaceType = face::FaceType<SiFace>;

#[derive(Debug)]
pub (crate) struct SiFace {
    pub (crate) rot: i8,
    pub (crate) rotated_image: DynamicImage,
}

#[derive(Clone)]
pub (crate) struct ViewSi {
    pub window: WindowProxy
}

impl face::ui::one::Face for SiFace {
    type View = ViewSi;

    fn load(path: impl AsRef<Path>) -> ferr::Result<SiFaceType>
    where
        Self: Sized,
    {
        let start = std::time::Instant::now();
        let image = image::open(&path)?;
        let elapsed_image = start.elapsed();

        let start = std::time::Instant::now();
        let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
        let elapsed_metadata = start.elapsed();

        println!("Loaded {path} in {elapsed_image:.0?} + {elapsed_metadata:.0?}",
                 path = path.as_ref().display()
        );

        let ui = Self {
            rot: face.rot,
            rotated_image: Self::image_rotated_by(&image, face.rot),
        };

        let path = path.as_ref().to_owned();
        ferr::Result::Ok(SiFaceType { path, face, ui })
    }

    // For non-blocking loading with preview
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
        face: &SiFaceType,
        view: &Self::View
    ) -> ferr::Result<()> {
        let FaceInImage { cx, cy, w, ..  } = face.face;
        let x = (cx - w / 2.0) as u32;
        let y = (cy - w / 2.0) as u32;
        let width = w as u32;
        let height = w as u32 * 5 / 4; // TODO replace magic number with ASPECT_RATIO
        let cropped = self.rotated_image.crop_imm(x, y, width, height);
        view.window.set_image("TODO label", cropped).unwrap();
        Ok(())
    }

    fn full_w(&self) -> ferr::Result<f32> { Ok(self.rotated_image.dimensions().0 as f32) }
    fn full_h(&self) -> ferr::Result<f32> { Ok(self.rotated_image.dimensions().1 as f32) }

    fn as_bytes(&self, face@&FaceInImage { cx, cy, w, .. }: &FaceInImage) -> Vec<u8> {
        let x = (cx - w / 2.0) as u32;
        let y = (cy - w / 2.0) as u32;
        let w =       w        as u32;
        let h = face.h()       as u32;
        self
            .rotated_image
            .crop_imm(x, y, w, h)
            .as_bytes()
            .to_owned()
    }

}

impl SiFace {
    pub fn image_rotated_by(image: &DynamicImage, rot: i8) -> DynamicImage {
        let rot = rot.rem_euclid(4);
        match rot {
            0 => image.clone(),
            1 => image.rotate90(),
            2 => image.rotate180(),
            3 => image.rotate270(),
            _ => unreachable!(),
        }
    }
}
