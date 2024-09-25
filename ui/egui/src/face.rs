use std::path::PathBuf;

use egui::{Context, TextureHandle, TextureOptions, Vec2};
use image::DynamicImage;

use face::{ui::one::Face, FaceInImage, ASPECT_RATIO};

pub (crate) struct CropEgui {
    pub path: PathBuf,
    pub face: FaceInImage,
    pub image: DynamicImage,
    pub texture: TextureHandle,
    pub texture_name: String,
}

impl CropEgui {
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &Context, selected: bool) {
        let w = ctx.available_rect().width();
        egui::Frame::none()
            .fill(if selected {egui::Color32::RED} else { egui::Color32::BLACK })
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(w / 6.5);
                    ui.vertical(|ui| {
                        let w = ui.available_width();
                        ui.add(egui::Image::new(&self.texture)
                               .max_size(Vec2 { x: w, y: w * ASPECT_RATIO }));
                    });
                    ui.horizontal(|ui| {
                        ui.label("prénom : ");
                        ui.text_edit_singleline(&mut self.face.given);
                    });
                    ui.horizontal(|ui| {
                        ui.label("nom : ");
                        ui.text_edit_singleline(&mut self.face.family);
                    });
                });
            });
    }

    pub fn rotate(&mut self, d_rot: i8, ctx: &Context) -> face::Result<i8> {
        self.face.rot = (self.face.rot + d_rot).rem_euclid(4);
        self.set_texture_from_cropped_image();
        Ok(self.face.rot)
    }

    pub fn set_texture_from_cropped_image(&mut self) {
        let cropped_image = crate::crop(&self.image, &self.face);
        let data = crate::crop_image_for_texture(&cropped_image);
        self.texture.set(data, TextureOptions::default());
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        crate::crop(&self.image, &self.face).as_bytes().to_owned()
    }
}

pub (crate) type FaceEgui<'i> = face::FaceType<CropEgui>;

#[derive(Debug)]
pub (crate) struct ViewEgui;

impl face::ui::one::Face for CropEgui {
    // TODO this cannot work, because eugi need &mut access to ui and ctx
    type View = ViewEgui;

    fn load(path: impl AsRef<std::path::Path>) -> face::Result<face::FaceType<Self>> where Self: Sized {
        todo!()
    }

    fn replace_image(&mut self, path: impl AsRef<std::path::Path>) -> face::Result<()> { todo!() }
    fn set_cx (&mut self, x: f32)                -> face::Result<f32> { todo!() }
    fn set_cy (&mut self, y: f32)                -> face::Result<f32> { todo!() }
    fn set_w  (&mut self, w: f32)                -> face::Result<f32> { todo!() }
    fn set_rot(&mut self, rot: i8)               -> face::Result< i8> { todo!() }
    fn full_w(&self)                             -> face::Result<f32> { todo!() }
    fn full_h(&self)                             -> face::Result<f32> { todo!() }
    fn as_bytes(&self, face: &face::FaceInImage) ->          Vec< u8> { todo!() }

    fn view(
        &self,
    // TODO this cannot work, because eugi need &mut access to ui and ctx
        face: &face::FaceType<Self>,
        view: &Self::View, // TODO this cannot work, because eugi need &mut access to ui and ctx
    ) -> face::Result<()>
    where Self: Sized
    {
        todo!()
    }

}
