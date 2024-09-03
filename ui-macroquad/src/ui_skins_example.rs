#![allow(unused)]

use macroquad::{
    prelude::*,
    ui::{hash, root_ui, widgets, Skin},
};

pub struct Share {
    pub default_skin: Skin,
    pub skin1: Skin,
    pub skin2: Skin,
    pub checkbox: bool,
    pub combobox: usize,
    pub text: String,
    pub number: f32,
}

pub fn ui_example(s: &mut Share) {

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

pub async fn skin1() -> Skin {
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

pub async fn skin2() -> Skin {
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
