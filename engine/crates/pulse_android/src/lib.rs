//! Archivo: engine/crates/pulse_android/src/lib.rs
//! Propósito: Puente JNI limpio entre Kotlin Jetpack Compose (`NativeEngineBridge.kt`)
//! y el motor modular en Rust (`pulse_core`, `pulse_audio`, `pulse_scripting`).
//! No usa `GameActivity`, `eframe` ni `winit`: todo el renderizado de interfaz vive en
//! Kotlin Compose y toda la lógica de motor, audio, XML, SVG, compilación y scripts vive en Rust.

pub mod session;

use crate::session::EngineSession;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::OnceLock;

static SESSION: OnceLock<Mutex<EngineSession>> = OnceLock::new();

pub fn init_session(cache_dir: PathBuf) -> &'static Mutex<EngineSession> {
    SESSION.get_or_init(|| Mutex::new(EngineSession::new(cache_dir)))
}

pub fn session() -> &'static Mutex<EngineSession> {
    SESSION.get_or_init(|| Mutex::new(EngineSession::new(std::env::temp_dir())))
}

#[cfg(target_os = "android")]
mod jni_exports {
    use super::*;
    use jni::JNIEnv;
    use jni::objects::{JByteArray, JClass, JString};
    use jni::sys::{jboolean, jdouble, jfloat, jint};

    fn read_str(env: &mut JNIEnv, input: &JString) -> String {
        env.get_string(input)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn to_jstr<'local>(env: &mut JNIEnv<'local>, s: &str) -> JString<'local> {
        env.new_string(s).unwrap_or_default()
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeInit<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        cache_dir: JString<'local>,
    ) -> JString<'local> {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("BeatForgeEngine"),
        );
        let dir = PathBuf::from(read_str(&mut env, &cache_dir));
        let sess = init_session(dir);
        let json = sess.lock().snapshot_json_string();
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetSnapshot<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = session().lock().snapshot_json_string();
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateProjectMeta<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        name: JString<'local>,
        author: JString<'local>,
        downscroll: jboolean,
        audio_offset_ms: jdouble,
        scroll_speed: jdouble,
    ) -> JString<'local> {
        let name_s = read_str(&mut env, &name);
        let author_s = read_str(&mut env, &author);
        let json = {
            let mut guard = session().lock();
            guard.project.name = name_s;
            guard.project.author = author_s;
            guard.project.downscroll = downscroll != 0;
            guard.project.audio_offset_ms = audio_offset_ms.clamp(-500.0, 500.0);
            guard.project.chart.speed = scroll_speed.clamp(0.5, 5.0);
            guard.ok_response("Proyecto actualizado")
        };
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportChart<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        path: JString<'local>,
    ) -> JString<'local> {
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session().lock().import_chart(p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestoreDemoChart<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = {
            let mut guard = session().lock();
            guard.project.chart = pulse_core::ChartData::bundled_demo_chart();
            guard.project.chart_source_path = None;
            guard.ok_response("Chart demo restaurado")
        };
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetChartBpm<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        bpm: jdouble,
    ) -> JString<'local> {
        let json = session().lock().set_chart_bpm(bpm);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeExportChartJson<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = session().lock().export_chart_json();
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateChartDetails<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        title: JString<'local>,
        speed: jdouble,
        player1: JString<'local>,
        player2: JString<'local>,
        gf_version: JString<'local>,
        stage: JString<'local>,
        needs_voices: jboolean,
    ) -> JString<'local> {
        let t = read_str(&mut env, &title);
        let p1 = read_str(&mut env, &player1);
        let p2 = read_str(&mut env, &player2);
        let gf = read_str(&mut env, &gf_version);
        let st = read_str(&mut env, &stage);
        let json = session().lock().update_chart_details(
            &t,
            speed,
            &p1,
            &p2,
            &gf,
            &st,
            needs_voices != 0,
        );
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetSectionBpm<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        sec_index: jint,
        bpm: jdouble,
        change_bpm: jboolean,
    ) -> JString<'local> {
        let json = session().lock().set_section_bpm(
            sec_index.max(0) as usize,
            bpm,
            change_bpm != 0,
        );
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddChartEvent<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        time_ms: jdouble,
        name: JString<'local>,
        value1: JString<'local>,
        value2: JString<'local>,
    ) -> JString<'local> {
        let n = read_str(&mut env, &name);
        let v1 = read_str(&mut env, &value1);
        let v2 = read_str(&mut env, &value2);
        let json = session().lock().add_chart_event(time_ms, &n, &v1, &v2);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRemoveChartEvent<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        index: jint,
    ) -> JString<'local> {
        let json = session().lock().remove_chart_event(index.max(0) as usize);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddChartSection<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        must_hit: jboolean,
    ) -> JString<'local> {
        let json = session().lock().add_chart_section(must_hit != 0);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleSectionMustHit<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        sec_index: jint,
    ) -> JString<'local> {
        let json = session()
            .lock()
            .toggle_section_must_hit(sec_index.max(0) as usize);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleChartNote<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        sec_index: jint,
        step_in_sec: jint,
        col_0_to_7: jint,
        sustain_steps: jint,
    ) -> JString<'local> {
        let json = session().lock().toggle_chart_note(
            sec_index.max(0) as usize,
            step_in_sec.max(0) as u32,
            col_0_to_7.clamp(0, 7) as u8,
            sustain_steps.max(0) as u32,
        );
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportAudioTrack<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        slot: jint,
        path: JString<'local>,
    ) -> JString<'local> {
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session().lock().import_audio_track(slot.max(0) as usize, p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRemoveAudioTrack<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        slot: jint,
    ) -> JString<'local> {
        let json = session().lock().remove_audio_track(slot.max(0) as usize);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateAudioTrackConfig<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        slot: jint,
        gain: jfloat,
        muted: jboolean,
        duck_on_miss: jboolean,
    ) -> JString<'local> {
        let json = session().lock().update_audio_track_config(
            slot.max(0) as usize,
            gain,
            muted != 0,
            duck_on_miss != 0,
        );
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleAudioPreview(
        _env: JNIEnv,
        _cls: JClass,
        play: jboolean,
        start_ms: jdouble,
    ) -> jboolean {
        let mut guard = session().lock();
        if play != 0 {
            guard.audio_engine.ensure_started();
            let any_audio = guard.project.active_audio_count() > 0;
            guard
                .audio_engine
                .mixer
                .use_demo_synth
                .store(!any_audio, std::sync::atomic::Ordering::Relaxed);
            guard.audio_engine.mixer.demo_bpm_bits.store(
                (guard.project.chart.bpm as f32).to_bits(),
                std::sync::atomic::Ordering::Relaxed,
            );
            guard.audio_engine.mixer.seek_ms(start_ms.max(0.0));
            guard.audio_engine.mixer.play();
            1
        } else {
            guard.audio_engine.mixer.pause();
            0
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportSpriteSheetPng<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        path: JString<'local>,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session().lock().import_sprite_png(&eid, p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportSparrowXml<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        path: JString<'local>,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session().lock().import_sparrow_xml(&eid, p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetSparrowBinding<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        anim_kind: JString<'local>,
        prefix: JString<'local>,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let kind = read_str(&mut env, &anim_kind);
        let pref = read_str(&mut env, &prefix);
        let json = session().lock().set_sparrow_binding(&eid, &kind, &pref);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetSparrowFrameInfo<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        anim_kind: JString<'local>,
        elapsed_sec: jfloat,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let kind = read_str(&mut env, &anim_kind);
        let json = session()
            .lock()
            .get_sparrow_frame_info(&eid, &kind, elapsed_sec);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddStageEntity<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        name: JString<'local>,
        role: JString<'local>,
    ) -> JString<'local> {
        let name_s = read_str(&mut env, &name);
        let role_s = read_str(&mut env, &role);
        let json = session().lock().add_stage_entity(&name_s, &role_s);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateStageEntity<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        name: JString<'local>,
        role: JString<'local>,
        dx: jfloat,
        dy: jfloat,
        abs_x: jfloat,
        abs_y: jfloat,
        use_abs_pos: jboolean,
        scale: jfloat,
        opacity: jfloat,
        visible: jboolean,
        flip_x: jboolean,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let name_s = read_str(&mut env, &name);
        let role_s = read_str(&mut env, &role);
        let json = session().lock().update_stage_entity(
            &eid,
            &name_s,
            &role_s,
            dx,
            dy,
            abs_x,
            abs_y,
            use_abs_pos != 0,
            scale,
            opacity,
            visible != 0,
            flip_x != 0,
        );
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeReorderStageEntity<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        entity_id: JString<'local>,
        action: JString<'local>,
    ) -> JString<'local> {
        let eid = read_str(&mut env, &entity_id);
        let act = read_str(&mut env, &action);
        let json = session().lock().reorder_stage_entity(&eid, &act);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddScript<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        language: JString<'local>,
    ) -> JString<'local> {
        let lang = read_str(&mut env, &language);
        let json = session().lock().add_script(&lang);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportScript<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        path: JString<'local>,
    ) -> JString<'local> {
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session().lock().import_script(p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateScript<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        script_id: JString<'local>,
        enabled: jboolean,
        source_code: JString<'local>,
    ) -> JString<'local> {
        let sid = read_str(&mut env, &script_id);
        let src = read_str(&mut env, &source_code);
        let json = session().lock().update_script(&sid, enabled != 0, src);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeValidateScript<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        script_id: JString<'local>,
    ) -> JString<'local> {
        let sid = read_str(&mut env, &script_id);
        let json = session().lock().validate_script(&sid);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportArrowSvg<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        direction: jint,
        path: JString<'local>,
    ) -> JString<'local> {
        let p = PathBuf::from(read_str(&mut env, &path));
        let json = session()
            .lock()
            .import_arrow_svg((direction.clamp(0, 3)) as u8, p);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestoreDefaultArrowSvg<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = session().lock().restore_default_arrow_svg();
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetArrowPngBytes<'local>(
        env: JNIEnv<'local>,
        _cls: JClass<'local>,
        direction: jint,
    ) -> JByteArray<'local> {
        let bytes = session()
            .lock()
            .rasterize_arrow_png_bytes((direction.clamp(0, 3)) as u8);
        env.byte_array_from_slice(&bytes).unwrap_or_default()
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeHotCompile<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        tier_code: jint,
    ) -> JString<'local> {
        let json = session().lock().hot_compile(tier_code);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStartGameplay<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = session().lock().start_gameplay();
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStepGameplay<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        dt_sec: jfloat,
        lane_pressed_mask: jint,
        lane_just_pressed_mask: jint,
    ) -> JString<'local> {
        let json = session()
            .lock()
            .step_gameplay(dt_sec, lane_pressed_mask, lane_just_pressed_mask);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetGameplayPaused<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
        paused: jboolean,
    ) -> JString<'local> {
        let json = session().lock().set_gameplay_paused(paused != 0);
        to_jstr(&mut env, &json)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStopGameplay<'local>(
        mut env: JNIEnv<'local>,
        _cls: JClass<'local>,
    ) -> JString<'local> {
        let json = session().lock().stop_gameplay();
        to_jstr(&mut env, &json)
    }
}
