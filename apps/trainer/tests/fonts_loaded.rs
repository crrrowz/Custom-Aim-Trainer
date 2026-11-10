use bevy::prelude::*;
use trainer::fonts::{
    setup_ui_fonts, UiFonts, ARABIC_REGULAR_BYTES, ARABIC_SEMIBOLD_BYTES, INTER_REGULAR_BYTES,
    INTER_SEMIBOLD_BYTES,
};

fn verify_font_bytes_and_glyphs(bytes: &[u8], name: &str, sample: &str) {
    assert!(
        bytes.len() > 12,
        "Font {} byte length too short: {}",
        name,
        bytes.len()
    );

    // Verify magic bytes: 00 01 00 00 (TrueType) or OTTO / true (OpenType)
    let magic = &bytes[0..4];
    let is_valid_magic = magic == [0x00, 0x01, 0x00, 0x00] || magic == b"OTTO" || magic == b"true";

    assert!(
        is_valid_magic,
        "Font {} has invalid magic bytes: {:02X?}",
        name, magic
    );

    // Verify ttf-parser parses the bytes successfully
    let face = ttf_parser::Face::parse(bytes, 0)
        .unwrap_or_else(|e| panic!("ttf-parser failed parsing {}: {:?}", name, e));

    // Verify all required sample glyphs exist in the font face
    for ch in sample.chars() {
        if ch == ' ' {
            continue;
        }
        let glyph_id = face.glyph_index(ch);
        assert!(
            glyph_id.is_some(),
            "Font {} is missing glyph for character '{}' (U+{:04X})",
            name,
            ch,
            ch as u32
        );
    }
}

#[test]
fn test_embedded_font_files_magic_bytes_and_glyph_coverage() {
    verify_font_bytes_and_glyphs(INTER_REGULAR_BYTES, "Inter-Regular", "PAUSED Resume");
    verify_font_bytes_and_glyphs(INTER_SEMIBOLD_BYTES, "Inter-SemiBold", "PAUSED Resume");
    verify_font_bytes_and_glyphs(
        ARABIC_REGULAR_BYTES,
        "IBMPlexSansArabic-Regular",
        "إيقاف مؤقت استئناف",
    );
    verify_font_bytes_and_glyphs(
        ARABIC_SEMIBOLD_BYTES,
        "IBMPlexSansArabic-SemiBold",
        "إيقاف مؤقت استئناف",
    );
}

#[test]
fn test_ui_fonts_resource_loaded_with_non_default_handles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Font>();
    app.add_systems(Startup, setup_ui_fonts);

    app.update();

    let ui_fonts = app
        .world()
        .get_resource::<UiFonts>()
        .expect("UiFonts resource must be present in world");

    assert_ne!(
        ui_fonts.regular,
        Handle::<Font>::default(),
        "Inter-Regular handle must not be default"
    );
    assert_ne!(
        ui_fonts.semibold,
        Handle::<Font>::default(),
        "Inter-SemiBold handle must not be default"
    );
    assert_ne!(
        ui_fonts.arabic_regular,
        Handle::<Font>::default(),
        "IBMPlexSansArabic-Regular handle must not be default"
    );
    assert_ne!(
        ui_fonts.arabic_semibold,
        Handle::<Font>::default(),
        "IBMPlexSansArabic-SemiBold handle must not be default"
    );

    let font_assets = app.world().resource::<Assets<Font>>();
    assert!(font_assets.get(&ui_fonts.regular).is_some());
    assert!(font_assets.get(&ui_fonts.semibold).is_some());
    assert!(font_assets.get(&ui_fonts.arabic_regular).is_some());
    assert!(font_assets.get(&ui_fonts.arabic_semibold).is_some());
}
