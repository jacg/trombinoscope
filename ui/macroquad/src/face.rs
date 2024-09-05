use std::f32::consts::TAU;

use macroquad::prelude::*;

use ::face::{
    FaceInImage,
    error as ferr,
};

pub const ASPECT_RATIO: f32 = 5.0 / 4.0;

pub (crate) type FaceMq = face::Face<CropMq>;

pub (crate) struct CropMq { pub (crate) texture: Texture2D }

pub (crate) struct ViewMq {
    pub (crate) col: usize,
    pub (crate) row: usize,
    pub (crate) col_w: f32,
    pub (crate) row_h: f32,
    pub (crate) color: Color,
}

impl face::CropUi for CropMq {
    type View = ViewMq;
    fn replace_image(&mut self, path: impl AsRef<std::path::Path>) -> face::error::Result<()> { todo!() }
    fn set_cx (&mut self, x: f32)  -> ferr::Result<f32> { Ok(x) }
    fn set_cy (&mut self, y: f32)  -> ferr::Result<f32> { Ok(y) }
    fn set_w  (&mut self, w: f32)  -> ferr::Result<f32> { Ok(w) }
    fn set_rot(&mut self, rot: i8) -> ferr::Result<i8>  { Ok(rot) }
    fn save  (&self)               -> ferr::Result<()>  { Err(ferr::Error::Todo) }
    fn full_w(&self)               -> ferr::Result<f32> { Ok(self.texture.size().x) }
    fn full_h(&self)               -> ferr::Result<f32> { Ok(self.texture.size().y) }
    fn view(
        &self,
        &FaceInImage { cx, cy, w, rot, .. }: &FaceInImage,
        &Self::View { col, row, col_w, row_h, color }: &Self::View
    ) -> face::error::Result<()> {

        let (full_w, full_h, rotate) = {
            let Vec2 { x, y } = self.texture.size();
            if x < y {(x, y, 0)} else {(y, x, 3)}
        } ;

        let h = w * ASPECT_RATIO;
        let x_ =          cx - w/2.;
        let xi = full_w - cx - w/2.;
        let y_ =          cy - h/2.;
        let yi = full_h - cy - h/2.;

        let (     x , y ,   w, h,   dest_x, dest_y,   dx   , dy   ) = match rot {
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
            &self.texture, x_pos, y_pos, color,
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

}
