use macroquad::prelude::*;

use util::find_jpgs_in_dir;

mod face;
use face::{ASPECT_RATIO, CropMq, FaceMq, ViewMq};

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
            if is_key_down   (G        ) { face.zoom_in   ( d); }
            if is_key_down   (P        ) { face.zoom_in   (-d); }
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
            face.view(ViewMq { col, row, col_w, row_h, color: if n == face_n { WHITE } else { GRAY }} );
        }

        //ui_example(&mut state);

        next_frame().await;
    }
    println!("TODO implement: Saving images.")
}
