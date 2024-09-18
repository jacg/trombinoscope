use egui::{Context, TextureHandle};

pub (crate) struct CropEgui {
    pub given: String,
    pub family: String,
    pub image: TextureHandle,
}

impl CropEgui {
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let w = ctx.available_rect().width();
        ui.vertical_centered(|ui| {
            ui.set_width(w / 6.0);
            ui.image(&self.image);
            ui.label(&self.given);
            ui.label(&self.family);
            ui.text_edit_singleline(&mut self.given);
        });
    }
}

pub (crate) type FaceEgui<'i> = face::FaceType<CropEgui>;

#[derive(Debug)]
pub (crate) struct ViewEgui;

impl face::ui::one::Face for CropEgui {
    type View = ViewEgui;

    fn load(path: impl AsRef<std::path::Path>) -> face::Result<face::FaceType<Self>> where Self: Sized {
        todo!()
    }

    fn replace_image(&mut self, path: impl AsRef<std::path::Path>) -> face::Result<()> { todo!() }
    fn set_cx (&mut self, x: f32)                       -> face::Result<f32> { todo!() }
    fn set_cy (&mut self, y: f32)                       -> face::Result<f32> { todo!() }
    fn set_w  (&mut self, w: f32)                       -> face::Result<f32> { todo!() }
    fn set_rot(&mut self, rot: i8)                      -> face::Result< i8> { todo!() }
    fn full_w(&self)                                    -> face::Result<f32> { todo!() }
    fn full_h(&self)                                    -> face::Result<f32> { todo!() }
    fn as_bytes(&self, face: &face::FaceInImage)        ->          Vec< u8> { todo!() }

    fn view(
        &self,
        face: &face::FaceType<Self>,
        view: &Self::View
    ) -> face::Result<()>
    where Self: Sized
    {
        todo!()
    }

}
