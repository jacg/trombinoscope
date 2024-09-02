use typst::{
    text::Font,
    foundations::Bytes,
};

pub fn fonts() -> Vec<Font> {
    let bytes = include_bytes!("../../fonts/Inconsolata-Black.ttf");
    let buffer = Bytes::from_static(bytes);
    vec![Font::new(buffer, 0).unwrap()]
}

pub fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    if let Ok(ok) = f() {
        Ok(ok)
    } else {
        f()
    }
}

pub fn http_successful(status: u16) -> bool {
    // 2XX
    status / 100 == 2
}
