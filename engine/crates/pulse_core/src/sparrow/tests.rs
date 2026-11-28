//! Archivo: engine/crates/pulse_core/src/sparrow/tests.rs
//! Propósito: Pruebas unitarias de normalización idempotente de Sparrow XML y alineación
//! por píxeles opacos de pies/suelo incluyendo fotogramas rotados.

use super::*;

#[test]
fn normalizes_and_centers_misaligned_sparrow_animations_idempotently() {
    let xml = r#"
    <TextureAtlas imagePath="hero.png">
        <SubTexture name="hero idle0000" x="0" y="0" width="200" height="300" frameX="-20" frameY="-10" frameWidth="240" frameHeight="320" />
        <SubTexture name="hero idle0001" x="200" y="0" width="196" height="304" frameX="-22" frameY="-6" frameWidth="240" frameHeight="320" />
        <SubTexture name="hero singLEFT0000" x="400" y="0" width="260" height="290" frameX="-260" frameY="-80" frameWidth="620" frameHeight="420" />
        <SubTexture name="hero singUP0000" x="700" y="0" width="180" height="360" />
    </TextureAtlas>
    "#;

    let mut atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    let idle = &atlas.animations["hero idle"].frames;
    let left = &atlas.animations["hero singLEFT"].frames;
    let up = &atlas.animations["hero singUP"].frames;

    // Todos comparten el mismo lienzo lógico unificado (260x360).
    assert_eq!(idle[0].frame_width, 260);
    assert_eq!(idle[0].frame_height, 360);
    assert_eq!(left[0].frame_width, 260);
    assert_eq!(left[0].frame_height, 360);
    assert_eq!(up[0].frame_width, 260);
    assert_eq!(up[0].frame_height, 360);

    // singLEFT queda centrado en X (0..260 -> frame_x=0) y al suelo en Y (360-290=70 -> frame_y=-70)
    assert_eq!(left[0].frame_x, 0);
    assert_eq!(left[0].frame_y, -70);

    // singUP sin frameX/Y queda centrado en X ((260-180)/2=40 -> frame_x=-40) y Y=0
    assert_eq!(up[0].frame_x, -40);
    assert_eq!(up[0].frame_y, 0);

    // Idle queda centrado horizontalmente ((260-200)/2=30 -> frame_x=-30) y pies abajo
    assert_eq!(idle[0].frame_x, -30);
    assert_eq!(idle[0].frame_y, -60);
    assert_eq!(idle[1].frame_y, -56);

    // Idempotencia al volver a ejecutar sobre el atlas ya normalizado
    let before_fx = idle[0].frame_x;
    let before_fy = idle[0].frame_y;
    atlas.normalize_and_center_animations();
    assert_eq!(atlas.animations["hero idle"].frames[0].frame_x, before_fx);
    assert_eq!(atlas.animations["hero idle"].frames[0].frame_y, before_fy);
}

#[test]
fn align_animations_with_rgba_locks_asymmetric_poses_and_rotated_frames_to_feet() {
    // Lienzo RGBA de 500x200 para 3 poses con brazos largos asimétricos, margen vacío abajo y rotación.
    let img_w = 500_u32;
    let img_h = 200_u32;
    let mut rgba = vec![0_u8; (img_w * img_h * 4) as usize];

    let mut paint_rect = |rx: u32, ry: u32, rw: u32, rh: u32| {
        for py in ry..(ry + rh) {
            for px in rx..(rx + rw) {
                let idx = ((py * img_w + px) * 4) as usize;
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    };

    // Frame 1: idle (x=0..80, y=0..120), pies centrados en lx=40, suelo en ly=120
    paint_rect(25, 10, 30, 110); // cuerpo+botas lx=25..55 -> centro=40, suelo=120

    // Frame 2: singRIGHT (x=100..260, y=0..150), botas en lx=20..50 (centro=35) y suelo en ly=110
    // + cabello/brazo largo en lx=50..155 (ly=20..70) + 40px de margen vacío abajo (ly=110..150)
    paint_rect(120, 10, 30, 100); // botas lx=20..50, ly=10..110
    paint_rect(150, 20, 105, 50); // brazo largo hacia la derecha

    // Frame 3: singLEFT ROTADO (rotated=true, rect en tex: x=300, y=0, width=120, height=140 -> vis 140x120)
    // Recordar transform rotación: visual (lx, ly) -> tex (x + width - 1 - ly, y + lx)
    // Queremos en visual (0..140 x 0..120): botas en lx=95..125 (centro=110), ly=10..115 (suelo=115),
    // brazo izquierdo largo en lx=0..95 (ly=20..65).
    for ly in 10..115 {
        for lx in 95..125 {
            let tx = 300 + (120 - 1 - ly);
            let ty = lx;
            let idx = ((ty * img_w + tx) * 4) as usize;
            rgba[idx] = 255;
            rgba[idx + 1] = 255;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = 255;
        }
    }
    for ly in 20..65 {
        for lx in 0..95 {
            let tx = 300 + (120 - 1 - ly);
            let ty = lx;
            let idx = ((ty * img_w + tx) * 4) as usize;
            rgba[idx] = 255;
            rgba[idx + 1] = 255;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = 255;
        }
    }

    let xml = r#"
    <TextureAtlas imagePath="miku.png">
        <SubTexture name="idle0000" x="0" y="0" width="80" height="120" />
        <SubTexture name="right0000" x="100" y="0" width="160" height="150" />
        <SubTexture name="left0000" x="300" y="0" width="120" height="140" rotated="true" />
    </TextureAtlas>
    "#;

    let mut atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    atlas.align_animations_with_rgba(img_w, img_h, &rgba);

    let f_idle = &atlas.animations["idle"].frames[0];
    let f_right = &atlas.animations["right"].frames[0];
    let f_left = &atlas.animations["left"].frames[0];

    assert_eq!(f_idle.frame_width, f_right.frame_width);
    assert_eq!(f_idle.frame_width, f_left.frame_width);
    assert_eq!(f_idle.frame_height, f_right.frame_height);
    assert_eq!(f_idle.frame_height, f_left.frame_height);

    let unified_center_x = (f_idle.frame_width as f32) * 0.5;
    let floor_idle = (-f_idle.frame_y as f32) + 120.0;
    let floor_right = (-f_right.frame_y as f32) + 110.0;
    let floor_left = (-f_left.frame_y as f32) + 115.0;

    // Suelo exacto al píxel pese a 40px de margen vacío en singRIGHT y rotación en singLEFT
    assert!((floor_idle - floor_right).abs() <= 1.0);
    assert!((floor_idle - floor_left).abs() <= 1.0);

    // Pies anclados al centro horizontal del lienzo en las 3 poses
    let feet_x_idle = (-f_idle.frame_x as f32) + 40.0;
    let feet_x_right = (-f_right.frame_x as f32) + 35.0;
    let feet_x_left = (-f_left.frame_x as f32) + 110.0;
    assert!((feet_x_idle - unified_center_x).abs() <= 1.0);
    assert!((feet_x_right - unified_center_x).abs() <= 1.0);
    assert!((feet_x_left - unified_center_x).abs() <= 1.0);

    // Idempotente en segunda pasada
    let snapshot = (
        f_idle.frame_x,
        f_idle.frame_y,
        f_right.frame_x,
        f_right.frame_y,
        f_left.frame_x,
        f_left.frame_y,
    );
    atlas.align_animations_with_rgba(img_w, img_h, &rgba);
    assert_eq!(
        (
            atlas.animations["idle"].frames[0].frame_x,
            atlas.animations["idle"].frames[0].frame_y,
            atlas.animations["right"].frames[0].frame_x,
            atlas.animations["right"].frames[0].frame_y,
            atlas.animations["left"].frames[0].frame_x,
            atlas.animations["left"].frames[0].frame_y,
        ),
        snapshot
    );
}

#[test]
fn gf_dancing_beat_binds_as_idle_and_loops() {
    let xml = r#"
    <TextureAtlas imagePath="GF_assets.png">
        <SubTexture name="GF Cheer0000" x="0" y="0" width="120" height="140" />
        <SubTexture name="GF Dancing Beat0000" x="120" y="0" width="120" height="140" />
        <SubTexture name="GF Dancing Beat0001" x="240" y="0" width="120" height="140" />
        <SubTexture name="GF sad0000" x="360" y="0" width="120" height="140" />
    </TextureAtlas>
    "#;

    let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
    assert_eq!(
        atlas.bindings.get(&CharacterAnimKind::Idle).map(String::as_str),
        Some("GF Dancing Beat")
    );
    assert!(atlas.is_idle_looped());
    assert_eq!(atlas.frame_count(CharacterAnimKind::Idle), 2);
}
