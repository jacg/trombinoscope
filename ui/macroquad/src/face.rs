use std::f32::consts::TAU;
use futures::executor::block_on;

use macroquad::prelude::*;

use ::face::{
    ASPECT_RATIO,
    FaceInImage,
    error as ferr,
    ui,
};

pub (crate) type FaceMq = face::FaceType<CropMq>;

pub (crate) struct CropMq { pub (crate) image: Texture2D }

pub (crate) struct ViewMq {
    pub (crate) col: usize,
    pub (crate) row: usize,
    pub (crate) col_w: f32,
    pub (crate) row_h: f32,
    pub (crate) color: Color,
}

impl ui::one::Face for CropMq {
    type View = ViewMq;
    fn load(path: impl AsRef<std::path::Path>) -> ferr::Result<FaceMq>
    where
        Self: Sized,
    {
        let start = std::time::Instant::now();
        let image: Texture2D = block_on(async {load_texture(&path.as_ref().to_string_lossy()).await } ).unwrap();
        let elapsed_image = start.elapsed();

        let start = std::time::Instant::now();
        let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
        let elapsed_metadata = start.elapsed();

        println!("Loaded {path} in {elapsed_image:.0?} + {elapsed_metadata:.0?}",
                 path = path.as_ref().display()
        );

        Ok(FaceMq {
            path: path.as_ref().to_owned(),
            face,
            ui: Self { image },
        })
    }

    fn replace_image(&mut self, path: impl AsRef<std::path::Path>) -> face::Result<()> { todo!() }
    fn set_cx (&mut self, x: f32)  -> ferr::Result<f32> { Ok(x) }
    fn set_cy (&mut self, y: f32)  -> ferr::Result<f32> { Ok(y) }
    fn set_w  (&mut self, w: f32)  -> ferr::Result<f32> { Ok(w) }
    fn set_rot(&mut self, rot: i8) -> ferr::Result<i8>  { Ok(rot) }
    fn full_w(&self)               -> ferr::Result<f32> { Ok(self.image.size().x) }
    fn full_h(&self)               -> ferr::Result<f32> { Ok(self.image.size().y) }
    fn view(
        &self,
        face: &FaceMq,
        &Self::View { col, row, col_w, row_h, color }: &Self::View
    ) -> face::Result<()> {

        let &FaceInImage { cx, cy, w, rot, .. } = &face.face;

        let (full_w, full_h, rotate) = {
            let Vec2 { x, y } = self.image.size();
            if x < y {(x, y, 0)} else {(y, x, 3)}
        } ;

        let h = w * face::ASPECT_RATIO;
        let x_ =          cx - w/2.;
        let xi = full_w - cx - w/2.;
        let y_ =          cy - h/2.;
        let yi = full_h - cy - h/2.;

        let (     x , y ,   w, h,   dest_x, dest_y,   dx   , dy   ) = match rot.rem_euclid(4) {
            0 => (x_, y_,   w, h,   col_w , row_h ,   col_w, row_h),
            1 => (y_, xi,   h, w,   row_h , col_w ,   row_h, col_w),
            2 => (xi, yi,   w, h,   col_w , row_h ,   col_w, row_h),
            3 => (yi, x_,   h, w,   row_h , col_w ,   row_h, col_w),
            _ => unreachable!(),
        };

        let x_piv = col_w * (col as f32 + 0.5);
        let y_piv = row_h * (row as f32 + 0.5);
        let x_pos = x_piv - dx/2.;
        let y_pos = y_piv - dy/2.;

        draw_texture_ex(
            &self.image, x_pos, y_pos, color,
            DrawTextureParams {
                dest_size: Some( Vec2 { x: dest_x, y: dest_y }),
                source: Some(Rect { x, y, w, h, }),
                rotation: rot as f32 * TAU/4.0,
                pivot: Some(Vec2 { x: x_piv , y: y_piv }),
                flip_x: false, flip_y: false,
            }
        );
        Ok(())
    }

    fn as_bytes(&self, &FaceInImage { cx, cy, w, .. }: &FaceInImage) -> Vec<u8> {
        let x = cx - w / 2.0;
        let y = cy - w / 2.0;
        let h = w * ASPECT_RATIO;
        let rgba_bytes = self
            .image
            .get_texture_data()
            .sub_image(Rect { x , y, w, h })
            .bytes;
        let mut rgb_bytes = Vec::with_capacity(rgba_bytes.len() * 3 / 4 + 1);
        for rgba in rgba_bytes.chunks(4) {
            rgb_bytes.extend(&rgba[..3]);
        }
        rgb_bytes
    }

}

use macroquad::texture::Image;
fn rotate(Image { mut bytes, width, height }: Image, rot: i8) -> Image {
    match rot.rem_euclid(4) {
        0 => Image { bytes, width, height },
        1 => {
            for c in 0..width {
                for r in 0..height {
                    todo!()
                    //(bytes[4*c..4*(c+1)], bytes[4*c..4*(c+1)])
                }
            }
        todo!()},
        2 => { bytes.reverse(); Image { bytes, width, height }; todo!() },
        3 => todo!(),
        _ => unreachable!(),
    }
}
