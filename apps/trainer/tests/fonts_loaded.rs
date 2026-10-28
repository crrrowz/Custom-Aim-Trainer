use bevy::prelude::*;
use trainer::fonts::{setup_ui_fonts, UiFonts};

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
