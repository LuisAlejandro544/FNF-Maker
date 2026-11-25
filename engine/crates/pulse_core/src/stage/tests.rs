//! Archivo: engine/crates/pulse_core/src/stage/tests.rs
//! Propósito: Pruebas unitarias de `pulse_core::stage` (autoajuste de sprites/atlas,
//! sistema multicámara, clamping contra fondos intercambiados, viñeta y limpieza de entidades).

use super::*;
use crate::sparrow::SparrowAtlas;
use std::path::PathBuf;

#[test]
fn stage_import_sizes_characters_and_props_generously_without_sheet_squashing() {
    let mut char_ent = StageEntity::new("opp", "Oponente", EntityRole::Opponent);
    // Primero importa un spritesheet ancho 4096x2048
    char_ent.fit_dimensions_to_image(4096, 2048);
    assert!(char_ent.width >= 400.0);

    // Luego importa su XML con pose Idle de 240x360 y ataque ancho de 480x380
    let xml = r#"
    <TextureAtlas imagePath="opp.png">
        <SubTexture name="opp idle0000" x="0" y="0" width="240" height="360" />
        <SubTexture name="opp singLEFT0000" x="240" y="0" width="480" height="380" />
    </TextureAtlas>
    "#;
    char_ent.atlas = Some(SparrowAtlas::from_xml_str(xml).unwrap());
    char_ent.fit_dimensions_to_atlas();

    // Debe conservar altura cómoda (~380px para el Idle) y proporción real del lienzo unificado 480x380
    assert!(char_ent.height >= 360.0 && char_ent.height <= 420.0);
    assert!(char_ent.width > char_ent.height);

    // Un fondo importado siempre cubre mínimo 1280x720
    let mut bg = StageEntity::new("bg2", "Fondo", EntityRole::Background);
    bg.fit_dimensions_to_image(800, 600);
    assert!(bg.width >= 1280.0 && bg.height >= 720.0);
}

#[test]
fn multi_camera_slots_resolve_targets_and_protect_base_three() {
    let mut stage = StageScene::default();
    stage.camera.ensure_default_cameras();
    // Desactivamos el recorte al fondo 1280x720 para validar las coordenadas crudas de cada ancla en zoom 1.0
    // (el recorte activo se prueba en `swapped_active_entities_clamp_camera_without_exposing_black_bars`).
    stage.camera.clamp_to_background = false;
    assert_eq!(stage.camera.cameras.len(), 3);

    // Cam 0 fija en (640, 360)
    let (x0, y0, z0, _) = stage.resolve_camera_slot_target(0, "", "");
    assert_eq!((x0, y0, z0), (640.0, 360.0, 1.0));

    // Cam 1 sigue al Player (980, 420) + offset (0, -24)
    let (x1, y1, _, _) = stage.resolve_camera_slot_target(1, "player_main", "opponent_main");
    assert!((x1 - 980.0).abs() < 0.01);
    assert!((y1 - 396.0).abs() < 0.01);

    // Cam 2 sigue al Enemigo (300, 415) + offset (0, -24)
    let (x2, y2, _, _) = stage.resolve_camera_slot_target(2, "player_main", "opponent_main");
    assert!((x2 - 300.0).abs() < 0.01);
    assert!((y2 - 391.0).abs() < 0.01);

    // Crear Cam 3 personalizada apuntando a la Cabina/GF con zoom 1.35
    let idx3 = stage.add_custom_camera(
        "Cam 3 · Cabina",
        CameraTargetKind::Gf,
        None,
        0.0,
        -30.0,
        1.35,
        8.0,
        1.0,
    );
    assert_eq!(idx3, 3);
    let (x3, y3, z3, _) = stage.resolve_camera_slot_target(3, "", "");
    assert!((x3 - 640.0).abs() < 0.01);
    assert!((y3 - 385.0).abs() < 0.01);
    assert!((z3 - 1.35).abs() < 0.01);

    // No permite borrar Cam 0..2, sí permite borrar Cam 3
    assert!(!stage.remove_custom_camera(0));
    assert!(!stage.remove_custom_camera(2));
    assert!(stage.remove_custom_camera(3));
    assert_eq!(stage.camera.cameras.len(), 3);
}

#[test]
fn swapped_active_entities_clamp_camera_without_exposing_black_bars() {
    let mut stage = StageScene::default();
    let mut bg_alt = StageEntity::new("bg_red", "Fondo Alterno", EntityRole::Background);
    bg_alt.x = 980.0;
    bg_alt.y = 360.0;
    bg_alt.width = 1600.0;
    bg_alt.height = 720.0;
    bg_alt.visible = false;
    bg_alt.image_path = Some(PathBuf::from("sprites/bg_red.png"));
    stage.entities.push(bg_alt);

    // Simula `active_entities` durante gameplay tras `Stage Swap`:
    // se oculta `stage_bg` ([0..1280]) y se muestra `bg_red` ([180..1780]).
    let mut active_entities = stage.entities.clone();
    active_entities[0].visible = false;
    active_entities.last_mut().unwrap().visible = true;

    let (clamped_x, clamped_y) =
        stage.clamp_camera_to_entities_background(240.0, 360.0, 1.0, &active_entities);
    // Con zoom 1.0 (half_vw = 640), el límite izquierdo de bg_red (180) exige camera_x >= 820.
    assert!((clamped_x - 820.0).abs() < 0.01);
    assert!((clamped_y - 360.0).abs() < 0.01);
}

#[test]
fn camera_vignette_sanitizes_color_bounds_and_runtime_sync() {
    let mut stage = StageScene::default();
    let mut cfg = stage.camera.clone();
    cfg.vignette_enabled = true;
    cfg.vignette_color = "crimson".to_string();
    cfg.vignette_alpha = 1.8;
    cfg.vignette_width = 0.01;
    cfg.vignette_pulse = 0.72;
    cfg.vignette_over_hud = true;

    stage.apply_camera_config(cfg);

    assert_eq!(stage.camera.vignette_color, "#DC2626");
    assert_eq!(stage.vignette_color_argb, 0xFFDC2626);
    assert!((stage.vignette_alpha - 1.0).abs() < 0.001);
    assert!((stage.vignette_width - 0.05).abs() < 0.001);
    assert!((stage.vignette_pulse - 0.72).abs() < 0.001);
    assert!(stage.vignette_over_hud);

    let mut off_cfg = stage.camera.clone();
    off_cfg.vignette_enabled = false;
    stage.apply_camera_config(off_cfg);
    assert_eq!(stage.vignette_alpha, 0.0);
}

#[test]
fn removing_and_clearing_stage_entities_cleans_camera_entity_targets() {
    let mut stage = StageScene::default();
    stage.camera.ensure_default_cameras();
    let initial_len = stage.entities.len();
    assert!(initial_len >= 4);

    let custom_cam_idx = stage.add_custom_camera(
        "Cam 3 · Fondo",
        CameraTargetKind::Entity,
        Some("stage_bg".to_string()),
        0.0,
        0.0,
        1.1,
        6.0,
        1.0,
    );

    let removed = stage.remove_entity("stage_bg").expect("stage_bg debe existir");
    assert_eq!(removed.id, "stage_bg");
    assert_eq!(stage.entities.len(), initial_len - 1);
    assert!(stage.remove_entity("stage_bg").is_none());

    let cam3 = &stage.camera.cameras[custom_cam_idx];
    assert_eq!(cam3.target_kind, CameraTargetKind::Fixed);
    assert!(cam3.target_entity_id.is_none());

    let wiped = stage.clear_entities();
    assert_eq!(wiped.len(), initial_len - 1);
    assert!(stage.entities.is_empty());
}
