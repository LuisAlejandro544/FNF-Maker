//! Archivo: engine/crates/pulse_core/src/hot_compile/tests.rs
//! Propósito: Pruebas unitarias del compilador en caliente (escala inteligente por tier,
//! caché incremental WebP, preservación de entidades referenciadas en eventos y reempaquetado Sparrow).

use super::*;
use crate::project::ModProject;
use crate::sparrow::SparrowAtlas;
use crate::stage::{EntityRole, StageEntity};
use image::{DynamicImage, Rgba, RgbaImage};

#[test]
fn smart_scale_skips_optimal_sprites_and_downscales_oversized_ones() {
    let mut prop = StageEntity::new("bg", "Fondo", EntityRole::Background);
    prop.width = 1280.0;
    prop.height = 720.0;
    prop.scale = 1.0;

    assert_eq!(
        compute_smart_texture_scale(&prop, 1920, 1080, DeviceResourceTier::Balanced),
        1.0
    );
    // Incluso en gama económica (LowRam32Bit), un fondo Full HD 1920x1080 se conserva 1:1 nítido
    assert_eq!(
        compute_smart_texture_scale(&prop, 1920, 1080, DeviceResourceTier::LowRam32Bit),
        1.0
    );

    let low_ratio =
        compute_smart_texture_scale(&prop, 4096, 2048, DeviceResourceTier::LowRam32Bit);
    assert!(low_ratio <= 1920.0 / 4096.0 + 0.001);

    let mut icon = StageEntity::new("icon", "Decoración", EntityRole::Decoration);
    icon.width = 200.0;
    icon.height = 200.0;
    icon.scale = 1.0;
    let ratio = compute_smart_texture_scale(&icon, 4096, 4096, DeviceResourceTier::Balanced);
    assert!(ratio < 0.6);
}

#[test]
fn incremental_webp_cache_reuses_valid_hits_and_invalidates_on_tier_or_source_change() {
    let base_dir = std::env::temp_dir().join("beatforge_incremental_cache_test");
    let _ = std::fs::remove_dir_all(&base_dir);
    let cache_dir = base_dir.join("compiled_cache");
    std::fs::create_dir_all(&base_dir).unwrap();

    let src_png = base_dir.join("hero.png");
    let img = RgbaImage::from_pixel(64, 64, Rgba([20, 180, 240, 255]));
    DynamicImage::ImageRgba8(img)
        .save_with_format(&src_png, image::ImageFormat::Png)
        .unwrap();

    let mut project = ModProject::new_demo();
    project.stage.entities.clear();
    let mut hero = StageEntity::new("hero_1", "Hero", EntityRole::Player);
    hero.image_path = Some(src_png.clone());
    hero.width = 220.0;
    hero.height = 220.0;
    project.stage.entities.push(hero);

    // 1ra compilación: debe codificar .png -> .webp sin pérdida
    let b1 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b1.report.webp_encoded_textures, 1);
    assert_eq!(b1.report.cached_textures_reused, 0);
    let cached_webp = b1.compiled_texture_paths.get("hero_1").unwrap();
    assert_eq!(cached_webp.extension().and_then(|e| e.to_str()), Some("webp"));
    assert!(cached_webp.exists());

    // 2da compilación idéntica: Cache Hit sin recodificar
    let b2 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b2.report.webp_encoded_textures, 0);
    assert_eq!(b2.report.cached_textures_reused, 1);

    // Cambio de Tier: debe invalidar caché y recompilar
    let b3 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::LowRam32Bit,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b3.report.webp_encoded_textures, 1);
    assert_eq!(b3.report.cached_textures_reused, 0);

    // Limpiar
    let _ = std::fs::remove_dir_all(&base_dir);
}

#[test]
fn chart_events_preserve_hidden_entities_and_detect_reused_later_sprites() {
    let mut project = ModProject::new_demo();
    let mut alt_bg = StageEntity::new("bg_night", "Fondo Noche", EntityRole::Background);
    alt_bg.visible = false;
    project.stage.entities.push(alt_bg);
    project.scripts.clear();
    project.chart.events.push(crate::chart::ChartEvent {
        time_ms: 10000.0,
        name: "Stage Swap".to_string(),
        value1: "show=bg_night;hide=bg_day;dispose=1".to_string(),
        value2: String::new(),
    });
    project.chart.events.push(crate::chart::ChartEvent {
        time_ms: 25000.0,
        name: "Stage Swap".to_string(),
        value1: "show=bg_day;hide=bg_night;dispose=0".to_string(),
        value2: String::new(),
    });

    let refs = collect_script_referenced_ids(&project);
    assert!(refs.contains("bg_night"));
    assert!(!is_permanently_invisible(
        project
            .stage
            .entities
            .iter()
            .find(|e| e.id == "bg_night")
            .unwrap(),
        &refs,
        1280.0,
        720.0,
    ));

    let reused = analyze_reused_entity_ids(&project);
    assert!(reused.contains("bg_day"));
    assert!(!reused.contains("bg_night"));
}

#[test]
fn hot_compile_repacks_atlas_and_reuses_repacked_atlas_on_cache_hit() {
    let base_dir = std::env::temp_dir().join("beatforge_atlas_repack_cache_test");
    let _ = std::fs::remove_dir_all(&base_dir);
    let cache_dir = base_dir.join("compiled_cache");
    std::fs::create_dir_all(&base_dir).unwrap();

    let src_png = base_dir.join("singer.png");
    let mut img = RgbaImage::new(512, 256);
    for y in 20..50 {
        for x in 20..40 {
            img.put_pixel(x, y, Rgba([220, 40, 120, 255]));
            img.put_pixel(x + 128, y, Rgba([220, 40, 120, 255]));
        }
    }
    DynamicImage::ImageRgba8(img)
        .save_with_format(&src_png, image::ImageFormat::Png)
        .unwrap();

    let xml = r#"
    <TextureAtlas imagePath="singer.png">
        <SubTexture name="singer idle0000" x="0" y="0" width="64" height="64" />
        <SubTexture name="singer idle0001" x="128" y="0" width="64" height="64" />
        <SubTexture name="singer idle0002" x="256" y="0" width="64" height="64" />
    </TextureAtlas>
    "#;
    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();

    let mut project = ModProject::new_demo();
    project.stage.entities.clear();
    let mut singer = StageEntity::new("singer_1", "Singer", EntityRole::Player);
    singer.image_path = Some(src_png);
    singer.atlas = Some(atlas);
    singer.width = 280.0;
    singer.height = 380.0;
    project.stage.entities.push(singer);

    // 1ra compilación: deduplica idle0001, elimina frame muerto idle0002, recorta alfa y reempaqueta
    let b1 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b1.report.repacked_atlases, 1);
    assert_eq!(b1.report.dead_frames_removed, 1);
    assert_eq!(b1.report.duplicate_frames_merged, 1);
    assert_eq!(b1.report.trimmed_frames, 2);
    assert_eq!(b1.report.repacked_entity_ids, vec!["singer_1".to_string()]);
    let compiled_atlas_1 = b1.active_entities[0].atlas.as_ref().unwrap();
    assert_eq!(compiled_atlas_1.animations["singer idle"].frames.len(), 3);
    assert_eq!(compiled_atlas_1.animations["singer idle"].frames[0].width, 20);
    assert_eq!(
        compiled_atlas_1.animations["singer idle"].frames[1].x,
        compiled_atlas_1.animations["singer idle"].frames[0].x
    );
    assert_eq!(compiled_atlas_1.animations["singer idle"].frames[2].width, 1);

    // 2da compilación (Cache Hit): conserva exactamente las coordenadas del atlas reempaquetado
    let b2 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b2.report.cached_textures_reused, 1);
    assert_eq!(b2.report.repacked_atlases, 1);
    assert_eq!(b2.report.duplicate_frames_merged, 1);
    assert_eq!(b2.report.dead_frames_removed, 1);
    let compiled_atlas_2 = b2.active_entities[0].atlas.as_ref().unwrap();
    assert_eq!(
        compiled_atlas_2.animations["singer idle"].frames[0].x,
        compiled_atlas_1.animations["singer idle"].frames[0].x
    );
    assert_eq!(
        compiled_atlas_2.animations["singer idle"].frames[2].width,
        1
    );

    let _ = std::fs::remove_dir_all(&base_dir);
}
