use std::f32::consts::TAU;

use macroquad::prelude::*;

use util::find_jpgs_in_dir;

use face::{
    face::FaceInImage,
    error as ferr,
};

const ASPECT_RATIO: f32 = 5.0 / 4.0;

mod ui_skins_example;

#[macroquad::main("Trombinoscope")]
async fn main() {

    use ui_skins_example::{skin1, skin2, Share};
    let skin1 = skin1().await;
    let skin2 = skin2().await;
    #[allow(unused)]
    let mut state =  Share {
        default_skin: macroquad::ui::root_ui().default_skin().clone(),
        skin1: skin1.clone(),
        skin2: skin2.clone(),
        checkbox: false,
        combobox: 0,
        text: "".into(),
        number: 0.0,
    };

    let path = cli::parse().class_dir.join("Complet");

    let mut faces = vec![];
    let start = std::time::Instant::now();
    for jpg in find_jpgs_in_dir(&path).into_iter() {
        let texture = load_texture(&jpg.to_string_lossy()).await.unwrap();
        let crop = CropMq { texture };
        faces.push(FaceMq::new(jpg, crop).unwrap())
    }
    println!("Loading of images took {:.0?}", start.elapsed());

    let mut face_n = 0;
    let n_faces = faces.len();
    loop {
        clear_background(GRAY);

        let mut d = 5.0;
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) { d *= 0.2; }

        {
            use KeyCode::*;
            let face = faces.get_mut(face_n).unwrap();
            if is_key_down   (Right    ) { face.move_right( d); }
            if is_key_down   (Left     ) { face.move_right(-d); }
            if is_key_down   (Down     ) { face.move_down ( d); }
            if is_key_down   (Up       ) { face.move_down (-d); }
            if is_key_down   (G        ) { face.enlarge   ( d); }
            if is_key_down   (P        ) { face.enlarge   (-d); }
            if is_key_pressed(R        ) { face.rotate    ( 1); }
            if is_key_pressed(L        ) { face.rotate    (-1); }
            if is_key_pressed(Space    ) && face_n < n_faces - 1 { face_n += 1; }
            if is_key_pressed(Backspace) && face_n > 0           { face_n -= 1; }
            if (is_key_down(LeftControl) || is_key_down(RightControl)) && is_key_down(Q) { break; }
        }

        let desired_w = screen_width() / 6.0;
        let desired_h = screen_height() / 4.0;
        let (col_w, row_h) = if desired_w * ASPECT_RATIO < desired_h {(desired_w, desired_w * ASPECT_RATIO)} else {(desired_h / ASPECT_RATIO, desired_h)};
        for (n, face) in faces.iter().enumerate() {
            let row = n / 6;
            let col = n % 6;
            face.render(ViewMq { col, row, col_w, row_h, color: if n == face_n { WHITE } else { GRAY }} );
        }

        //ui_example(&mut state);

        next_frame().await;
    }
    println!("TODO implement: Saving images.")
}

type FaceMq = face::face::Face<CropMq>;

struct CropMq { texture: Texture2D }

struct ViewMq {
    col: usize,
    row: usize,
    col_w: f32,
    row_h: f32,
    color: Color,
}

impl face::face::CropUi for CropMq {
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
