//! Archivo: engine/crates/pulse_core/src/atlas_repack/tests.rs
//! Propósito: Pruebas unitarias de `atlas_repack` verificando eliminación de frames muertos,
//! deduplicación, recorte alfa, preservación de animaciones referenciadas en scripts, escalado
//! adaptativo con `Lanczos3` y división automática multi-página de mega-atlas Sparrow.

use super::*;
use crate::sparrow::{CharacterAnimKind, SparrowAtlas};
use crate::stage::{EntityRole, StageEntity};
use image::{DynamicImage, Rgba, RgbaImage};
use std::collections::HashSet;

fn paint_solid(img: &mut RgbaImage, x0: u32, y0: u32, w: u32, h: u32, color: Rgba<u8>) {
    for y in y0..(y0 + h) {
        for x in x0..(x0 + w) {
            img.put_pixel(x, y, color);
        }
    }
}

#[test]
fn repack_eliminates_dead_frames_merges_duplicates_trims_alpha_and_prunes_unused() {
    // Hoja grande 1024x512 con:
    // - idle0000: bloque 20x30 con bordes transparentes enormes (80x100)
    // - idle0001: copia EXACTA de idle0000 pero con basura RGB invisible (alpha=0)
    // - idle0002: frame MUERTO 100% transparente (debe conservar duración 3 frames en idle)
    // - unused0000: animación huérfana sin bind ni mención en scripts
    let mut sheet = RgbaImage::new(1024, 512);
    paint_solid(&mut sheet, 30, 20, 20, 30, Rgba([255, 80, 20, 255]));
    paint_solid(&mut sheet, 200, 0, 80, 100, Rgba([12, 200, 99, 0]));
    paint_solid(&mut sheet, 230, 20, 20, 30, Rgba([255, 80, 20, 255]));
    paint_solid(&mut sheet, 500, 50, 240, 240, Rgba([40, 120, 240, 255]));

    let xml = r#"
    <TextureAtlas imagePath="hero.png">
        <SubTexture name="hero idle0000" x="0" y="0" width="80" height="100" />
        <SubTexture name="hero idle0001" x="200" y="0" width="80" height="100" />
        <SubTexture name="hero idle0002" x="400" y="0" width="80" height="100" />
        <SubTexture name="unused pose0000" x="500" y="50" width="240" height="240" />
    </TextureAtlas>
    "#;

    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    let mut ent = StageEntity::new("hero", "Hero", EntityRole::Player);
    ent.width = 280.0;
    ent.height = 380.0;
    ent.atlas = Some(atlas.clone());

    let keep_refs = HashSet::new();
    let res = optimize_and_repack_sparrow_atlas(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet),
        2560,
        Some(&keep_refs),
    )
    .unwrap();

    assert_eq!(res.dead_frames_removed, 1);
    assert_eq!(res.duplicate_frames_merged, 1);
    assert_eq!(res.trimmed_frames, 2);
    assert_eq!(res.pruned_unused_anims, 1);
    assert!(res.modified);
    assert!(res.vram_saved_bytes > 1_000_000);
    assert!(!res.atlas.animations.contains_key("unused pose"));

    let idle_frames = &res.atlas.animations["hero idle"].frames;
    assert_eq!(idle_frames.len(), 3);
    assert_eq!(idle_frames[0].x, idle_frames[1].x);
    assert_eq!(idle_frames[0].y, idle_frames[1].y);
    assert_eq!(idle_frames[0].width, 20);
    assert_eq!(idle_frames[0].height, 30);
    assert_eq!(idle_frames[1].width, 20);
    assert_eq!(idle_frames[1].height, 30);
    assert_eq!((idle_frames[2].x, idle_frames[2].y), (0, 0));
    assert_eq!((idle_frames[2].width, idle_frames[2].height), (1, 1));
    assert_eq!(res.image.to_rgba8().get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
    assert!(res.atlas.bindings.contains_key(&CharacterAnimKind::Idle));
}

#[test]
fn repack_preserves_script_referenced_unbound_animations_and_trim_offsets() {
    let mut sheet = RgbaImage::new(256, 256);
    paint_solid(&mut sheet, 15, 25, 10, 12, Rgba([200, 200, 200, 255]));
    paint_solid(&mut sheet, 100, 10, 16, 18, Rgba([10, 220, 120, 255]));

    let xml = r#"
    <TextureAtlas imagePath="hero.png">
        <SubTexture name="hero idle0000" x="0" y="0" width="40" height="50" />
        <SubTexture name="special hey0000" x="100" y="10" width="16" height="18" />
    </TextureAtlas>
    "#;
    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    let ent = StageEntity::new("hero", "Hero", EntityRole::Player);
    let orig_idle = &atlas.animations["hero idle"].frames[0];
    let orig_fx = orig_idle.frame_x;
    let orig_fy = orig_idle.frame_y;

    let mut keep_refs = HashSet::new();
    keep_refs.insert("special hey".to_string());

    let res = optimize_and_repack_sparrow_atlas(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet),
        2560,
        Some(&keep_refs),
    )
    .unwrap();

    assert_eq!(res.pruned_unused_anims, 0);
    assert!(res.atlas.animations.contains_key("special hey"));

    let new_idle = &res.atlas.animations["hero idle"].frames[0];
    assert_eq!(new_idle.frame_x, orig_fx - 15);
    assert_eq!(new_idle.frame_y, orig_fy - 25);
    assert_eq!(new_idle.width, 10);
    assert_eq!(new_idle.height, 12);
}

#[test]
fn repack_fits_oversized_unique_frames_inside_tier_cap() {
    let mut sheet = RgbaImage::new(1024, 1024);
    paint_solid(&mut sheet, 0, 0, 300, 300, Rgba([250, 40, 40, 255]));
    paint_solid(&mut sheet, 320, 0, 300, 300, Rgba([40, 250, 40, 255]));
    paint_solid(&mut sheet, 0, 320, 300, 300, Rgba([40, 40, 250, 255]));
    paint_solid(&mut sheet, 320, 320, 300, 300, Rgba([250, 250, 40, 255]));

    let xml = r#"
    <TextureAtlas imagePath="boss.png">
        <SubTexture name="boss idle0000" x="0" y="0" width="300" height="300" />
        <SubTexture name="boss idle0001" x="320" y="0" width="300" height="300" />
        <SubTexture name="boss idle0002" x="0" y="320" width="300" height="300" />
        <SubTexture name="boss idle0003" x="320" y="320" width="300" height="300" />
    </TextureAtlas>
    "#;
    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    let ent = StageEntity::new("boss", "Boss", EntityRole::Opponent);

    let res = optimize_and_repack_sparrow_atlas(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet),
        400,
        None,
    )
    .unwrap();

    assert!(res.image.width() <= 400);
    assert!(res.image.height() <= 400);
    assert!(res.frame_scale_applied < 1.0);
    assert_eq!(res.atlas.animations["boss idle"].frames.len(), 4);
}

#[test]
fn repack_downscales_oversized_frame_with_lanczos3_according_to_viewport_and_zoom() {
    let mut sheet = RgbaImage::new(1024, 1024);
    paint_solid(&mut sheet, 10, 10, 900, 900, Rgba([60, 180, 255, 255]));
    let xml = r#"
    <TextureAtlas imagePath="big_char.png">
        <SubTexture name="big idle0000" x="0" y="0" width="960" height="960" />
    </TextureAtlas>
    "#;
    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    let mut ent = StageEntity::new("big", "Big", EntityRole::Player);
    ent.width = 240.0;
    ent.height = 300.0;
    ent.scale = 1.0;

    let compact = optimize_and_repack_sparrow_atlas_for_viewport(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet.clone()),
        4096,
        None,
        0.667,
        1.0,
    )
    .unwrap();
    let zoomed_fhd = optimize_and_repack_sparrow_atlas_for_viewport(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet),
        4096,
        None,
        1.5,
        1.6,
    )
    .unwrap();

    assert!(compact.frame_scale_applied < zoomed_fhd.frame_scale_applied);
    assert!(compact.image.width() < zoomed_fhd.image.width());
}

#[test]
fn multi_page_sparrow_splitting_preserves_crispness_across_pages_inside_tier_cap() {
    let mut sheet = RgbaImage::new(2048, 1600);
    let mut sub_tags = String::new();
    for idx in 0..10_u32 {
        let col = idx % 5;
        let row = idx / 5;
        let sx = col * 405;
        let sy = row * 405;
        paint_solid(
            &mut sheet,
            sx,
            sy,
            390,
            390,
            Rgba([
                30 + (idx as u8) * 20,
                200 - (idx as u8) * 12,
                90 + (idx as u8) * 14,
                255,
            ]),
        );
        sub_tags.push_str(&format!(
            r#"<SubTexture name="boss idle{:04}" x="{}" y="{}" width="390" height="390" />"#,
            idx, sx, sy
        ));
    }

    let xml = format!(r#"<TextureAtlas imagePath="mega.png">{}</TextureAtlas>"#, sub_tags);
    let atlas = SparrowAtlas::from_xml_str(&xml).unwrap();
    let mut ent = StageEntity::new("mega_boss", "Mega Boss", EntityRole::Opponent);
    ent.width = 480.0;
    ent.height = 480.0;
    ent.scale = 1.0;

    let res = optimize_and_repack_sparrow_atlas_for_viewport(
        &ent,
        &atlas,
        &DynamicImage::ImageRgba8(sheet),
        1024,
        None,
        1.5,
        1.2,
    )
    .unwrap();

    assert!(res.page_count >= 3);
    assert_eq!(res.extra_pages.len(), res.page_count - 1);
    assert!((res.frame_scale_applied - 1.0).abs() < 0.001);
    assert!(res.image.width() <= 1024 && res.image.height() <= 1024);
    for extra in &res.extra_pages {
        assert!(extra.image.width() <= 1024 && extra.image.height() <= 1024);
    }
    let max_page = res.atlas.animations["boss idle"]
        .frames
        .iter()
        .map(|f| f.page_index)
        .max()
        .unwrap_or(0);
    assert_eq!(max_page as usize, res.page_count - 1);
}
