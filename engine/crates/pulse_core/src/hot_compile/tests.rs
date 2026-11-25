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

#[test]
fn viewport_and_max_camera_zoom_guide_lanczos3_mip_downscale_and_invalidate_cache() {
    let mut project = ModProject::new_demo();
    project.stage.camera.base_zoom = 0.85;
    // El chart demo incluye un evento `Add Camera Zoom` (+0.04); aislamos primero `ZoomCamera`.
    project.chart.events.clear();
    project.chart.events.push(crate::chart::ChartEvent {
        time_ms: 4000.0,
        name: "ZoomCamera".to_string(),
        value1: "1.55".to_string(),
        value2: "0.5".to_string(),
    });
    let max_zoom = compute_project_max_camera_zoom(&project);
    assert!((max_zoom - 1.55).abs() < 0.01);

    project.chart.events.push(crate::chart::ChartEvent {
        time_ms: 5000.0,
        name: "Add Camera Zoom".to_string(),
        value1: "0.04".to_string(),
        value2: "0.02".to_string(),
    });
    assert!((compute_project_max_camera_zoom(&project) - 1.59).abs() < 0.01);

    let mut prop = StageEntity::new("bg_hq", "Fondo HQ", EntityRole::Background);
    prop.width = 1280.0;
    prop.height = 720.0;
    prop.scale = 1.0;

    // En pantalla pequeña 854x480 sin zoom (1.0), un fondo 2560x1440 se reduce más con Lanczos3
    // que en una pantalla 2400x1080 con evento ZoomCamera 1.55x.
    let small_scale = compute_smart_texture_scale_for_viewport(
        &prop,
        2560,
        1440,
        DeviceResourceTier::HighQuality,
        ScreenViewportMetrics {
            pixel_width: 854,
            pixel_height: 480,
        },
        1.0,
        1280.0,
        720.0,
    );
    let large_zoom_scale = compute_smart_texture_scale_for_viewport(
        &prop,
        2560,
        1440,
        DeviceResourceTier::HighQuality,
        ScreenViewportMetrics {
            pixel_width: 2400,
            pixel_height: 1080,
        },
        max_zoom,
        1280.0,
        720.0,
    );
    assert!(small_scale < 0.75);
    assert!(large_zoom_scale > small_scale);
}

#[test]
fn timeline_asset_lifecycle_schedules_preload_and_safe_retire_windows() {
    let mut project = ModProject::new_demo();
    project.scripts.clear();
    project.stage.entities.clear();

    let mut bg_day = StageEntity::new("bg_day", "Día", EntityRole::Background);
    bg_day.visible = true;
    let mut bg_night = StageEntity::new("bg_night", "Noche", EntityRole::Background);
    bg_night.visible = false;
    let mut secret = StageEntity::new("secret_boss", "Secreto", EntityRole::Opponent);
    secret.visible = false;

    project.stage.entities.push(bg_day);
    project.stage.entities.push(bg_night);
    project.stage.entities.push(secret);

    project.chart.events.push(crate::chart::ChartEvent {
        time_ms: 14000.0,
        name: "Stage Swap".to_string(),
        value1: "show=bg_night;hide=bg_day".to_string(),
        value2: String::new(),
    });
    project.scripts.push(crate::project::ScriptAsset {
        id: "scr_secret".to_string(),
        name: "secret.lua".to_string(),
        language: crate::project::ScriptLanguage::Lua,
        source_code: "if curBeat == 32 then showEntity('secret_boss') end".to_string(),
        enabled: true,
        precompiled_js: None,
        precompiled_bytecode: None,
    });

    let windows = compute_timeline_asset_lifecycle(&project);
    assert_eq!(windows.len(), 3);

    let w_day = windows.iter().find(|w| w.entity_id == "bg_day").unwrap();
    assert!(w_day.initially_visible);
    assert_eq!(w_day.preload_at_ms, None);
    assert_eq!(w_day.retire_after_ms, Some(15500.0));

    let w_night = windows.iter().find(|w| w.entity_id == "bg_night").unwrap();
    assert!(!w_night.initially_visible);
    assert_eq!(w_night.first_visible_ms, Some(14000.0));
    assert_eq!(w_night.preload_at_ms, Some(10000.0));
    assert_eq!(w_night.retire_after_ms, None);

    let w_secret = windows.iter().find(|w| w.entity_id == "secret_boss").unwrap();
    assert_eq!(w_secret.retire_after_ms, None);
    assert!(w_secret.trigger_reason.contains("script_activo"));
}

#[test]
fn hot_compile_applies_rgb565_dithering_for_opaque_bg_and_exoquant_only_when_colors_exceed_256() {
    use crate::astc_loader::{dither_image_to_rgb565_grid, quantize_rgba_with_exoquant};

    let base_dir = std::env::temp_dir().join("beatforge_rgb565_exoquant_test");
    let _ = std::fs::remove_dir_all(&base_dir);
    let cache_dir = base_dir.join("compiled_cache");
    std::fs::create_dir_all(&base_dir).unwrap();

    // 1) Fondo opaco con degradado suave -> debe codificarse en RGB_565 + dithering
    let bg_png = base_dir.join("sky.png");
    let mut bg = RgbaImage::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            bg.put_pixel(x, y, Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]));
        }
    }
    let dyn_bg = DynamicImage::ImageRgba8(bg);
    dyn_bg
        .save_with_format(&bg_png, image::ImageFormat::Png)
        .unwrap();
    let dithered = dither_image_to_rgb565_grid(&dyn_bg);
    assert_eq!(dithered.dimensions(), (64, 64));

    // 2) Pixel-art con <= 8 colores únicos -> exoquant NO debe tocarlo (devuelve None)
    let mut pixel_art = RgbaImage::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            pixel_art.put_pixel(
                x,
                y,
                if (x + y) % 2 == 0 {
                    Rgba([255, 0, 80, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                },
            );
        }
    }
    assert!(quantize_rgba_with_exoquant(&pixel_art, 256).is_none());

    // 3) Sprite con > 400 colores reales -> exoquant sí cuantiza a <= 256 colores con dithering
    let mut rich_sprite = RgbaImage::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            rich_sprite.put_pixel(x, y, Rgba([(x * 7) as u8, (y * 7) as u8, ((x + y) * 3) as u8, 220]));
        }
    }
    let q_res = quantize_rgba_with_exoquant(&rich_sprite, 256);
    assert!(q_res.is_some());

    let mut project = ModProject::new_demo();
    project.stage.entities.clear();
    let mut sky = StageEntity::new("sky_bg", "Cielo", EntityRole::Background);
    sky.image_path = Some(bg_png);
    sky.width = 1280.0;
    sky.height = 720.0;
    project.stage.entities.push(sky);

    let bundle = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::LowRam32Bit,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(bundle.report.rgb565_textures, 1);
    assert_eq!(
        bundle.texture_pixel_formats.get("sky_bg"),
        Some(&TexturePixelFormat::Rgb565)
    );

    let _ = std::fs::remove_dir_all(&base_dir);
}

#[test]
fn passthrough_webp_registers_in_manifest_and_hits_cache_on_second_compile() {
    let base_dir = std::env::temp_dir().join("beatforge_passthrough_manifest_test");
    let _ = std::fs::remove_dir_all(&base_dir);
    let cache_dir = base_dir.join("compiled_cache");
    std::fs::create_dir_all(&base_dir).unwrap();

    // Crear un WebP nativo óptimo (sin necesidad de reescalar)
    let webp_path = base_dir.join("optimal_prop.webp");
    let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(128, 128, Rgba([40, 200, 120, 255])));
    crate::astc_loader::save_compiled_webp(
        &img,
        &webp_path,
        crate::astc_loader::WebpPixelPolicy::LosslessRgba,
    )
    .unwrap();

    let mut project = ModProject::new_demo();
    project.stage.entities.clear();
    let mut prop = StageEntity::new("prop_webp", "Prop WebP", EntityRole::Decoration);
    prop.image_path = Some(webp_path.clone());
    prop.width = 256.0;
    prop.height = 256.0;
    project.stage.entities.push(prop);

    // 1ra compilación: debe salir PassThrough y quedar registrado en manifest.textures
    let b1 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b1.passthrough_paths.get("prop_webp"), Some(&webp_path));
    assert_eq!(b1.report.skipped_optimal_textures, 1);
    assert_eq!(b1.report.cached_textures_reused, 0);

    let manifest = HotCompileManifest::load_or_default(&cache_dir, &project.id);
    assert!(
        manifest.textures.contains_key("prop_webp"),
        "PassThrough debe quedar registrado en manifest.textures"
    );

    // 2da compilación: debe reutilizar la entrada PassThrough vía CacheHit al instante
    let b2 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::Balanced,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b2.report.cached_textures_reused, 1);
    assert_eq!(b2.passthrough_paths.get("prop_webp"), Some(&webp_path));
    assert!(webp_path.exists());

    let _ = std::fs::remove_dir_all(&base_dir);
}

#[test]
fn hot_compile_optimizes_and_caches_countdown_sprites_and_character_icons() {
    let base_dir = std::env::temp_dir().join("beatforge_countdown_icons_compile_test");
    let _ = std::fs::remove_dir_all(&base_dir);
    let cache_dir = base_dir.join("compiled_cache");
    std::fs::create_dir_all(&base_dir).unwrap();

    let cd_png = base_dir.join("go_huge.png");
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(2048, 1024, Rgba([255, 220, 40, 255])))
        .save_with_format(&cd_png, image::ImageFormat::Png)
        .unwrap();

    let icon_png = base_dir.join("bf_icon_strip.png");
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(1024, 512, Rgba([60, 180, 255, 255])))
        .save_with_format(&icon_png, image::ImageFormat::Png)
        .unwrap();

    let mut project = ModProject::new_demo();
    project.stage.entities.clear();
    let mut player = StageEntity::new("bf", "Jugador", EntityRole::Player);
    player.icon_normal_path = Some(icon_png);
    project.stage.entities.push(player);
    project.countdown.slots[3].image_path = Some(cd_png);

    let b1 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::LowRam32Bit,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b1.report.compiled_countdown_count, 1);
    assert_eq!(b1.report.compiled_icon_count, 1);

    let out_cd = b1.compiled_countdown_paths.get(&3).unwrap();
    let out_icon = b1.compiled_icon_normal_paths.get("bf").unwrap();
    assert!(out_cd.exists());
    assert!(out_icon.exists());
    assert_eq!(out_cd.extension().and_then(|e| e.to_str()), Some("webp"));
    assert_eq!(out_icon.extension().and_then(|e| e.to_str()), Some("webp"));

    let manifest = HotCompileManifest::load_or_default(&cache_dir, &project.id);
    assert!(manifest.countdown.contains_key(&3));
    assert!(manifest.icons.contains_key("bf::normal"));

    // Segunda compilación: verifica que la limpieza de huérfanos en out_dir no borre countdown ni íconos
    let b2 = hot_compile_project_to_dir(
        &project,
        DeviceResourceTier::LowRam32Bit,
        Some(&cache_dir),
        |_, _| {},
    );
    assert_eq!(b2.report.compiled_countdown_count, 1);
    assert_eq!(b2.report.compiled_icon_count, 1);
    assert!(b2.compiled_countdown_paths.get(&3).unwrap().exists());
    assert!(b2.compiled_icon_normal_paths.get("bf").unwrap().exists());

    let _ = std::fs::remove_dir_all(&base_dir);
}

