//! Archivo: engine/crates/pulse_core/src/chart/tests.rs
//! Propósito: Pruebas unitarias de parseo multi-formato, edición manual, serialización
//! round-trip, eventos de cámara y configuración intro UI del módulo `chart`.

use super::*;

#[test]
fn manual_toggle_and_add_section_updates_stats() {
    let mut chart = ChartData::bundled_demo_chart();
    let initial_sections = chart.sections.len();
    let new_idx = chart.add_section(true);
    assert_eq!(new_idx, initial_sections);

    let placed = chart.toggle_note_at(new_idx, 4, 5, 2);
    assert!(placed);
    assert!(
        chart
            .notes
            .iter()
            .any(|n| n.is_player && n.direction == NoteDirection::Down && n.sustain_ms > 0.0)
    );

    let removed = chart.toggle_note_at(new_idx, 4, 5, 0);
    assert!(!removed);
}

#[test]
fn complete_chart_json_round_trip_preserves_metadata_events_and_lanes() {
    let mut chart = ChartData::bundled_demo_chart();
    chart.set_metadata(
        "Smoke Track",
        2.6,
        "bf_custom",
        "garcello_custom",
        "gf_none",
        "alley",
        true,
    );
    chart.set_section_bpm(1, 160.0, true);
    chart.add_event(2500.0, "Camera Flash", "0.5", "ffffff");

    let exported = chart.to_json_str();
    let reimported = ChartData::from_json_str(&exported).expect("reimportación válida");

    assert_eq!(reimported.title, "Smoke Track");
    assert_eq!(reimported.player1, "bf_custom");
    assert_eq!(reimported.player2, "garcello_custom");
    assert_eq!(reimported.gf_version, "gf_none");
    assert_eq!(reimported.stage, "alley");
    assert_eq!(reimported.sections[1].bpm, 160.0);
    assert!(reimported.sections[1].change_bpm);
    assert_eq!(reimported.notes.len(), chart.notes.len());
    assert!(reimported.events.iter().any(|e| e.name == "Camera Flash"));
}

#[test]
fn parses_vslice_difficulty_map_and_double_wrapped_and_bom_charts() {
    let vslice = "\u{FEFF}{\n  \"version\": \"2.0.0\",\n  \"songName\": \"V-Slice Track\",\n  \"bpm\": 150,\n  \"scrollSpeed\": { \"hard\": 2.8, \"normal\": 2.2 },\n  \"events\": [{ \"t\": 1200, \"e\": \"FocusCamera\", \"v\": { \"char\": 1 } }],\n  \"notes\": {\n    \"hard\": [\n      { \"t\": 500, \"d\": 0, \"l\": 0, \"k\": \"\" },\n      { \"t\": 1000, \"d\": 5, \"l\": 200, \"k\": \"alt\" }\n    ]\n  }\n}";
    let parsed = ChartData::from_json_str(vslice).expect("V-Slice válido");
    assert_eq!(parsed.title, "V-Slice Track");
    assert_eq!(parsed.bpm, 150.0);
    assert_eq!(parsed.speed, 2.8);
    assert_eq!(parsed.notes.len(), 2);
    assert!(parsed.notes[0].is_player);
    assert_eq!(parsed.notes[0].direction, NoteDirection::Left);
    assert!(!parsed.notes[1].is_player);
    assert_eq!(parsed.notes[1].direction, NoteDirection::Down);
    assert_eq!(parsed.notes[1].sustain_ms, 200.0);
    assert_eq!(parsed.events.len(), 1);
    assert!(parsed.sections.len() >= 4);

    let double_wrapped = r#"{
      "song": {
        "song": {
          "song": "Double Wrapped",
          "bpm": "140",
          "notes": [
            { "mustHit": true, "sectionNotes": [[400, 2, 0]] }
          ]
        }
      }
    }"#;
    let parsed_dw = ChartData::from_json_str(double_wrapped).expect("Double wrapped válido");
    assert_eq!(parsed_dw.title, "Double Wrapped");
    assert_eq!(parsed_dw.bpm, 140.0);
    assert_eq!(parsed_dw.notes.len(), 1);
    assert!(parsed_dw.notes[0].is_player);
}

#[test]
fn parses_codename_strumlines_chart() {
    let codename = r#"{
      "codenameChart": true,
      "bpm": 132,
      "scrollSpeed": 2.4,
      "events": [{ "name": "Camera Movement", "time": 800, "params": [1] }],
      "strumLines": [
        { "type": 0, "position": "dad", "notes": [{ "time": 500, "id": 1, "sLen": 0 }] },
        { "type": 1, "position": "boyfriend", "notes": [{ "time": 1000, "id": 3, "sLen": 150 }] }
      ]
    }"#;
    let parsed = ChartData::from_json_str(codename).expect("Codename válido");
    assert_eq!(parsed.bpm, 132.0);
    assert_eq!(parsed.speed, 2.4);
    assert_eq!(parsed.notes.len(), 2);
    assert!(!parsed.notes[0].is_player);
    assert_eq!(parsed.notes[0].direction, NoteDirection::Down);
    assert!(parsed.notes[1].is_player);
    assert_eq!(parsed.notes[1].direction, NoteDirection::Right);
    assert_eq!(parsed.notes[1].sustain_ms, 150.0);
    assert_eq!(parsed.events.len(), 1);
}

#[test]
fn chart_section_camera_slot_and_camera_events_parse_cleanly() {
    let mut chart = ChartData::bundled_demo_chart();
    chart.set_section_camera_slot(0, Some(3));
    assert_eq!(chart.camera_state_at_ms(10.0), (0, false, false, Some(3)));

    let exported = chart.to_json_str();
    let reimported = ChartData::from_json_str(&exported).expect("chart con cameraSlot");
    assert_eq!(reimported.sections[0].camera_slot, Some(3));

    assert_eq!(
        parse_chart_camera_event("Switch Camera", "cam=3;zoom=1.40;dur=0.5;ease=quadOut", "", 120.0),
        Some(ParsedChartCameraAction::SelectCamera {
            slot: Some(3),
            focus_gf: false,
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: Some(1.4),
            duration_sec: 0.5,
            ease: "quadOut".to_string(),
        })
    );
    assert_eq!(
        parse_chart_camera_event("Camera Follow Pos", "710", "320", 120.0),
        Some(ParsedChartCameraAction::FollowPos {
            x: 710.0,
            y: 320.0,
            zoom: None,
            duration_sec: 0.0,
            ease: "sineOut".to_string(),
        })
    );
    assert_eq!(
        parse_chart_camera_event("Camera Follow Pos", "", "", 120.0),
        Some(ParsedChartCameraAction::ClearFollowPos)
    );
    assert_eq!(
        parse_chart_camera_event(
            "FocusCamera",
            r#"{"char":1,"x":20,"y":-10,"duration":4}"#,
            "",
            120.0
        ),
        Some(ParsedChartCameraAction::SelectCamera {
            slot: Some(2),
            focus_gf: false,
            offset_x: 20.0,
            offset_y: -10.0,
            zoom: None,
            duration_sec: 0.5,
            ease: "sineOut".to_string(),
        })
    );
}

#[test]
fn chart_intro_ui_round_trips_and_parses_shorthands() {
    let mut chart = ChartData::bundled_demo_chart();
    chart.set_intro_ui(ChartIntroUiConfig {
        start_blackout: true,
        blackout_alpha: 0.9,
        hide_hud: true,
        hide_arrows: true,
        reveal_on_song_start: false,
        reveal_delay_ms: 450.0,
        fade_sec: 0.85,
    });

    let exported = chart.to_json_str();
    let reimported = ChartData::from_json_str(&exported).expect("chart con uiBlackout");
    assert!(reimported.intro_ui.start_blackout);
    assert!((reimported.intro_ui.blackout_alpha - 0.9).abs() < 0.01);
    assert!(reimported.intro_ui.hide_hud);
    assert!(reimported.intro_ui.hide_arrows);
    assert!(!reimported.intro_ui.reveal_on_song_start);
    assert!((reimported.intro_ui.reveal_delay_ms - 450.0).abs() < 0.1);
    assert!((reimported.intro_ui.fade_sec - 0.85).abs() < 0.01);

    let shorthand = r#"{
      "song": {
        "song": "Stealth Mod",
        "bpm": 140,
        "hideArrows": true,
        "hideHud": true,
        "startBlackout": false,
        "introRevealOnStart": false,
        "notes": [{ "mustHitSection": true, "sectionNotes": [[500, 0, 0]] }]
      }
    }"#;
    let parsed = ChartData::from_json_str(shorthand).expect("chart con shorthands");
    assert!(!parsed.intro_ui.start_blackout);
    assert!(parsed.intro_ui.hide_hud);
    assert!(parsed.intro_ui.hide_arrows);
    assert!(!parsed.intro_ui.reveal_on_song_start);
}
