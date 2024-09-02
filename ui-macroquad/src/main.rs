use std::{
    f32::consts::TAU,
    path::PathBuf,
};

use macroquad::{
    prelude::*,
    ui::{hash, root_ui, widgets, Skin},
};

use util::find_jpgs_in_dir;

const ASPECT_RATIO: f32 = 5.0 / 4.0;


#[macroquad::main("Trombinoscope")]
async fn main() {

    let skin1 = skin1().await;
    let skin2 = skin2().await;

    let mut state =  Share {
        default_skin: root_ui().default_skin().clone(),
        skin1: skin1.clone(),
        skin2: skin2.clone(),
        checkbox: false,
        combobox: 0,
        text: "".into(),
        number: 0.0,
    };

    let mut args = std::env::args();
    let _executable = args.next();
    let path = args.next().unwrap();

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

struct Share {
    default_skin: Skin,
    skin1: Skin,
    skin2: Skin,
    checkbox: bool,
    combobox: usize,
    text: String,
    number: f32,
}

fn ui_example(s: &mut Share) {

    let mut window1_skin = s.skin1.clone();
    let mut window2_skin = s.skin2.clone();

    root_ui().group(hash!(), vec2(70.0, 100.0), |ui| {
        ui.label(None, "Window 1");
        if ui.button(None, "Skin 1" ) { window1_skin = s.skin1.clone(); }
        if ui.button(None, "Skin 2" ) { window1_skin = s.skin2.clone(); }
        if ui.button(None, "No Skin") { window1_skin = s.default_skin.clone(); }
    });
    root_ui().same_line(0.);
    root_ui().group(hash!(), vec2(70.0, 100.0), |ui| {
        ui.label(None, "Window 2");
        if ui.button(None, "Skin 1" ) { window2_skin = s.skin1.clone(); }
        if ui.button(None, "Skin 2" ) { window2_skin = s.skin2.clone(); }
        if ui.button(None, "No Skin") { window2_skin = s.default_skin.clone(); }
    });

    root_ui().push_skin(&window1_skin);

    root_ui().window(hash!(), vec2(20., 250.), vec2(300., 300.), |ui| {
        widgets::Button::new("Play"   ).position(vec2(65.0,  15.0)).ui(ui);
        widgets::Button::new("Options").position(vec2(40.0,  75.0)).ui(ui);
        widgets::Button::new("Quit"   ).position(vec2(65.0, 195.0)).ui(ui);
    });
    root_ui().pop_skin();

    root_ui().push_skin(&window2_skin);
    root_ui().window(hash!(), vec2(250., 20.), vec2(500., 250.), |ui| {
        ui.checkbox   (hash!(), "Checkbox 1", &mut s.checkbox);
        ui.combo_box  (hash!(), "Combobox"  , &["First option", "Second option"], &mut s.combobox);
        ui.input_text (hash!(), "Text"      , &mut s.text);
        ui.drag       (hash!(), "Drag"      , None, &mut s.number);

        widgets::Button::new("Apply" ).position(vec2( 80.0, 150.0)).ui(ui);
        widgets::Button::new("Cancel").position(vec2(280.0, 150.0)).ui(ui);
    });
    root_ui().pop_skin();

}

async fn skin1() -> Skin {
    let font = load_ttf_font("fonts/Inconsolata-Black.ttf")
        .await
        .unwrap();
    let label_style = root_ui()
        .style_builder()
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(180, 180, 120, 255))
        .font_size(30)
        .build();

    let window_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(20.0, 20.0, 10.0, 10.0))
        .margin(RectOffset::new(-20.0, -30.0, 0.0, 0.0))
        .build();

    let button_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(37.0, 37.0, 5.0, 5.0))
        .margin(RectOffset::new(10.0, 10.0, 0.0, 0.0))
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(180, 180, 100, 255))
        .font_size(40)
        .build();

    let editbox_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(0., 0., 0., 0.))
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(120, 120, 120, 255))
        .color_selected(Color::from_rgba(190, 190, 190, 255))
        .font_size(50)
        .build();

    Skin {
        editbox_style,
        window_style,
        button_style,
        label_style,
        ..root_ui().default_skin()
    }
}

async fn skin2() -> Skin {
    let font = load_ttf_font("fonts/Inconsolata-Black.ttf")
        .await
        .unwrap();
    let label_style = root_ui()
        .style_builder()
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(120, 120, 120, 255))
        .font_size(25)
        .build();

    let window_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(52.0, 52.0, 52.0, 52.0))
        .margin(RectOffset::new(-30.0, 0.0, -30.0, 0.0))
        .build();

    let button_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(8.0, 8.0, 8.0, 8.0))
        .color_hovered(RED)
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(180, 180, 100, 255))
        .font_size(40)
        .build();

    let checkbox_style = root_ui()
        .style_builder()
        .build();

    let editbox_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(2., 2., 2., 2.))
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(120, 120, 120, 255))
        .font_size(25)
        .build();

    let combobox_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(4., 25., 6., 6.))
        .with_font(&font)
        .unwrap()
        .text_color(Color::from_rgba(120, 120, 120, 255))
        .color(Color::from_rgba(210, 210, 210, 255))
        .font_size(25)
        .build();

    Skin {
        window_style,
        button_style,
        label_style,
        checkbox_style,
        editbox_style,
        combobox_style,
        ..root_ui().default_skin()
    }
}
