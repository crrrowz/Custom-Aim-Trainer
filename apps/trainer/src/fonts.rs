use bevy::prelude::*;

pub const INTER_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub const INTER_SEMIBOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
pub const ARABIC_REGULAR_BYTES: &[u8] =
    include_bytes!("../assets/fonts/IBMPlexSansArabic-Regular.ttf");
pub const ARABIC_SEMIBOLD_BYTES: &[u8] =
    include_bytes!("../assets/fonts/IBMPlexSansArabic-SemiBold.ttf");

#[derive(Resource, Clone)]
pub struct UiFonts {
    pub regular: Handle<Font>,
    pub semibold: Handle<Font>,
    pub arabic_regular: Handle<Font>,
    pub arabic_semibold: Handle<Font>,
}

pub fn setup_ui_fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let regular = fonts.add(Font::from_bytes(INTER_REGULAR_BYTES.to_vec()));
    let semibold = fonts.add(Font::from_bytes(INTER_SEMIBOLD_BYTES.to_vec()));
    let arabic_regular = fonts.add(Font::from_bytes(ARABIC_REGULAR_BYTES.to_vec()));
    let arabic_semibold = fonts.add(Font::from_bytes(ARABIC_SEMIBOLD_BYTES.to_vec()));

    // Plex Arabic also carries Latin glyphs, so one font covers a whole Arabic screen.
    let (regular, semibold) = if crate::ui_tokens::strings::is_arabic() {
        (arabic_regular.clone(), arabic_semibold.clone())
    } else {
        (regular, semibold)
    };

    commands.insert_resource(UiFonts {
        regular,
        semibold,
        arabic_regular,
        arabic_semibold,
    });
}
