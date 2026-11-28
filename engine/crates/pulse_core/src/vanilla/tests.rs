//! Archivo: engine/crates/pulse_core/src/vanilla/tests.rs
//! Propósito: Pruebas unitarias de `pulse_core::vanilla` (clasificación de códecs, roundtrip
//! `.mod` con deduplicación + WebP lossless y rechazo de corrupción/path traversal).

use super::*;
use crate::astc_loader::load_sprite_image;
use crate::project::{ModProject, current_epoch_ms};
use image::RgbaImage;

#[test]
fn vanilla_codec_classification_compresses_text_wav_astc_and_stores_compressed_media() {
    assert_eq!(
        classify_vanilla_entry_codec("project.json"),
        (VanillaEntryCodec::Zstd, VANILLA_ZSTD_JSON_LEVEL)
    );
    assert_eq!(
        classify_vanilla_entry_codec("assets/audio/song.wav"),
        (VanillaEntryCodec::Zstd, VANILLA_ZSTD_DATA_LEVEL)
    );
    assert_eq!(
        classify_vanilla_entry_codec("assets/sprites/hero.astc"),
        (VanillaEntryCodec::Zstd, VANILLA_ZSTD_DATA_LEVEL)
    );
    assert_eq!(
        classify_vanilla_entry_codec("assets/audio/inst.ogg"),
        (VanillaEntryCodec::Raw, 0)
    );
    assert_eq!(
        classify_vanilla_entry_codec("assets/sprites/hero.webp"),
        (VanillaEntryCodec::Raw, 0)
    );
}

#[test]
fn vanilla_roundtrip_exports_and_imports_preserving_pixels_audio_and_dedup() {
    let tmp_root = std::env::temp_dir().join(format!("beatforge_vanilla_rt_{}", current_epoch_ms()));
    let src_assets = tmp_root.join("src_assets");
    let _ = std::fs::create_dir_all(&src_assets);

    // 1. Crear PNG con patrón sólido repetitivo (PNG -> WebP lossless encoge muy bien)
    let png_path = src_assets.join("stage_bg.png");
    let mut img = RgbaImage::new(128, 128);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([((x / 8) * 16) as u8, ((y / 8) * 16) as u8, 220, 255]);
    }
    image::DynamicImage::ImageRgba8(img.clone())
        .save(&png_path)
        .expect("guardar PNG fuente");

    // 2. Crear WAV silencioso/repetitivo (Zstd logra ratio brutal)
    let wav_path = src_assets.join("inst.wav");
    let mut wav_bytes = Vec::from(b"RIFF\x24\x40\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00\x80\xbb\x00\x00\x00\xee\x02\x00\x04\x00\x10\x00data\x00\x40\x00\x00".as_slice());
    wav_bytes.resize(wav_bytes.len() + 16384, 0u8);
    std::fs::write(&wav_path, &wav_bytes).expect("guardar WAV");

    // 3. Crear OGG simulado (debe guardarse Raw)
    let ogg_path = src_assets.join("voices.ogg");
    std::fs::write(&ogg_path, b"OggS_fake_compressed_stream_payload").expect("guardar OGG");

    let mut proj = ModProject::new_blank("proj_src", "Test Vanilla Mod", "Tester");
    proj.assign_audio_track(0, wav_path).unwrap();
    proj.assign_audio_track(1, ogg_path.clone()).unwrap();
    if let Some(bg) = proj.stage.entities.first_mut() {
        bg.raw_image_path = Some(png_path.clone());
        bg.image_path = Some(png_path.clone());
    }
    proj.countdown.slots[0].raw_image_path = Some(png_path.clone());
    proj.countdown.slots[0].image_path = Some(png_path);
    proj.countdown.slots[0].image_name = Some("ready.png".to_string());
    proj.countdown.slots[0].audio_path = Some(ogg_path);
    proj.countdown.slots[0].audio_name = Some("intro2.ogg".to_string());

    let mod_file = tmp_root.join("test_vanilla_mod.mod");
    let report = export_project_vanilla_mod(&proj, &mod_file, true).expect("exportación Vanilla");
    assert!(mod_file.is_file());
    assert!(report.archive_bytes < report.source_bytes);
    assert!(report.deduplicated_assets >= 2); // raw_image_path == image_path y countdown deduplicados

    let inspected = inspect_vanilla_mod(&mod_file).expect("inspección rápida");
    assert_eq!(inspected.container_name, "Vanilla");
    assert_eq!(inspected.project_name, "Test Vanilla Mod");

    let dst_dir = tmp_root.join("installed_proj");
    let imported = import_project_vanilla_mod(&mod_file, "proj_dst", &dst_dir)
        .expect("importación Vanilla");
    assert_eq!(imported.project.id, "proj_dst");
    assert_eq!(imported.project.name, "Test Vanilla Mod");
    assert!(imported.project.audio_tracks[0].file_path.as_ref().unwrap().is_file());
    assert!(imported.project.audio_tracks[1].file_path.as_ref().unwrap().is_file());
    assert!(imported.project.countdown.slots[0].image_path.as_ref().unwrap().is_file());
    assert!(imported.project.countdown.slots[0].audio_path.as_ref().unwrap().is_file());

    let imported_bg = imported.project.stage.entities[0]
        .image_path
        .as_ref()
        .expect("ruta imagen importada");
    let reloaded_img = load_sprite_image(imported_bg).expect("recarga WebP/PNG");
    assert_eq!(reloaded_img.to_rgba8().get_pixel(10, 20), img.get_pixel(10, 20));

    let _ = std::fs::remove_dir_all(&tmp_root);
}

#[test]
fn vanilla_rejects_corrupted_checksum_and_unsafe_paths() {
    assert!(validate_vanilla_rel_path("../escape.txt").is_err());
    assert!(validate_vanilla_rel_path("/abs/path.png").is_err());
    assert!(validate_vanilla_rel_path("C:\\win\\a.png").is_err());
    assert!(validate_vanilla_rel_path("assets/sprites/bf.webp").is_ok());

    let tmp_root = std::env::temp_dir().join(format!("beatforge_vanilla_corrupt_{}", current_epoch_ms()));
    let _ = std::fs::create_dir_all(&tmp_root);
    let proj = ModProject::new_demo();
    let mod_file = tmp_root.join("corrupt_test.mod");
    export_project_vanilla_mod(&proj, &mod_file, true).expect("export");

    let mut bytes = std::fs::read(&mod_file).expect("leer mod");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF; // Corromper último byte de datos
    std::fs::write(&mod_file, &bytes).expect("escribir mod corrupto");

    let res = import_project_vanilla_mod(&mod_file, "proj_bad", &tmp_root.join("bad_out"));
    assert!(res.is_err());

    let _ = std::fs::remove_dir_all(&tmp_root);
}
