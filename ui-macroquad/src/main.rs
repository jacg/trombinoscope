use std::{
    f32::consts::TAU,
    path::PathBuf,
};

use macroquad::prelude::*;

use util::find_jpgs_in_dir;

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
    for jpg in find_jpgs_in_dir(&path).into_iter().take(30) {
        faces.push(Face::new(&jpg.to_string_lossy()).await)
    }
    println!("Loading of images took {:.0?}", start.elapsed());

    let mut face_n = 0;
    let n_faces = faces.len();
    loop {
        clear_background(GRAY);

        let mut dx = 5.0;
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) { dx *= 0.2; }

        {
            use KeyCode::*;
            let face = faces.get_mut(face_n).unwrap();
            if is_key_down   (Left     ) { face.cx += dx; }
            if is_key_down   (Right    ) { face.cx -= dx; }
            if is_key_down   (Down     ) { face.cy -= dx; }
            if is_key_down   (Up       ) { face.cy += dx; }
            if is_key_down   (P        ) { face.w  += dx; }
            if is_key_down   (G        ) { face.w  -= dx; }
            if is_key_pressed(R        ) { face.rotate = (face.rotate + 1).rem_euclid(4); }
            if is_key_pressed(L        ) { face.rotate = (face.rotate - 1).rem_euclid(4); }
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
            face.render(col, row, col_w, row_h, if n == face_n { WHITE } else { GRAY });
        }

        //ui_example(&mut state);

        next_frame().await;
    }
    println!("TODO implement: Saving images.")
}


#[derive(Debug)]
pub struct Face {
    pub path: PathBuf,
    pub given: String,
    pub family: String,
    full_w: f32, full_h: f32,
    cx: f32, cy: f32,
    w: f32,
    /// Rotation applied to image in quarter-turns clockwise
    rotate: i8,
    texture: Texture2D,
}


impl Face {
    async fn new(path: &str) -> Self {
        let texture = load_texture(path).await.unwrap();
        let (full_w, full_h, rotate) = {
            let Vec2 { x, y } = texture.size();
            if x < y {(x, y, 0)} else {(y, x, 3)}
        } ;
        let (frac_cx, frac_cy, frac_w) = (0.5, 0.15, 0.18);
        let w  = frac_w *  full_w;
        let cx = frac_cx * full_w;
        let cy = frac_cy * full_h;
        Self {
            path: path.into(),
            given: "TODO Prénom".into(),
            family: "TODO Nom".into(),
            full_w, full_h,
            cx, cy, w,
            rotate,
            texture,
        }
    }

    fn render(&self, col: usize, row: usize, col_w: f32, row_h: f32, color: Color) {
        let Self { full_w, full_h, cx, cy, w, rotate, .. } = *self;
        let h = self.h();
        let x_ =          cx - w/2.;
        let xi = full_w - cx - w/2.;
        let y_ =          cy - h/2.;
        let yi = full_h - cy - h/2.;

        let (     x , y ,   w, h,   dest_x, dest_y,   dx   , dy   ) = match rotate {
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
                rotation: rotate as f32 * TAU/4.0,
                pivot: Some(Vec2 { x: x_piv , y: y_piv }),
                flip_x: false, flip_y: false,
            }
        );
    }

    fn h(&self) -> f32 { ASPECT_RATIO * self.w }
}
