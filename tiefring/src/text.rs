pub struct Font {}

impl Font {
    pub fn load_font() {
        let font = include_bytes!("../../sample/fonts/Roboto-Regular.ttf") as &[u8];
        let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();

        let (metrics, bitmap) = font.rasterize('c', 17.0);
    }
}
