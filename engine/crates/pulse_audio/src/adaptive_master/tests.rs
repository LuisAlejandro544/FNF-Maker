//! Archivo: engine/crates/pulse_audio/src/adaptive_master/tests.rs
//! Propósito: Pruebas unitarias del motor `adaptive_master` (EBU R128 + FFT,
//! perfiles por hardware/ruta, remuestreo Rubato, limitador Lookahead y flujo WAV f32).

use super::*;

fn make_sine_stereo(freq_hz: f32, sample_rate: u32, duration_sec: f32, amp: f32) -> Vec<f32> {
    let frames = (sample_rate as f32 * duration_sec) as usize;
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * amp;
        out.push(s);
        out.push(s);
    }
    out
}

#[test]
fn ebur128_and_realfft_distinguish_sub_bass_vs_bright_track() {
    // K-weighting (ITU-R BS.1770) atenúa ~3 dB a 60 Hz y realza ~4 dB a 3 kHz (~7 dB de sesgo).
    // Usamos 0.75 vs 0.15 (~14 dB de diferencia RMS) para que la pista grave sea claramente más fuerte en LUFS.
    let bass_track = make_sine_stereo(60.0, 48_000, 1.5, 0.75);
    let bright_track = make_sine_stereo(3000.0, 48_000, 1.5, 0.15);

    let a_bass = analyze_song_acoustics(&bass_track, 48_000);
    let a_bright = analyze_song_acoustics(&bright_track, 48_000);

    assert!(a_bass.bands.sub_bass_20_90_hz > 0.7);
    assert!(a_bright.bands.presence_2k_4k_hz > 0.7);
    assert!(a_bass.integrated_lufs > a_bright.integrated_lufs);
    assert!(a_bass.bands.spectral_tilt_db < 0.0);
    assert!(a_bright.bands.spectral_tilt_db > 0.0);
}

#[test]
fn adaptive_plan_changes_across_phone_routes_and_song_profiles() {
    let mono_phone = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput {
            route: AndroidOutputRoute::BuiltinSpeaker,
            manufacturer: "Xiaomi".to_string(),
            model: "Redmi Note".to_string(),
            is_mono_speaker: true,
            reported_channel_count: 1,
            os_sample_rate: 48_000,
            ..Default::default()
        },
        &AudioHardwareProbe {
            aaudio_active: true,
            sample_rate: 48_000,
            frames_per_burst: 192,
            ..Default::default()
        },
    );

    let headphones = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput {
            route: AndroidOutputRoute::WiredHeadphones,
            manufacturer: "Samsung".to_string(),
            model: "Galaxy S23".to_string(),
            reported_channel_count: 2,
            os_sample_rate: 48_000,
            ..Default::default()
        },
        &AudioHardwareProbe {
            aaudio_active: true,
            sample_rate: 48_000,
            frames_per_burst: 96,
            ..Default::default()
        },
    );

    let bt_a2dp = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput {
            route: AndroidOutputRoute::BluetoothA2dp,
            os_sample_rate: 44_100,
            ..Default::default()
        },
        &AudioHardwareProbe {
            aaudio_active: true,
            sample_rate: 44_100,
            ..Default::default()
        },
    );

    let sub_heavy_song = SongAcousticAnalysis {
        sample_rate: 44_100,
        duration_sec: 120.0,
        integrated_lufs: -11.0,
        loudness_range_lu: 5.0,
        true_peak_dbtp: -0.2,
        crest_factor_db: 7.5,
        stereo_correlation: 0.4,
        bands: SpectralBandProfile {
            sub_bass_20_90_hz: 0.40,
            bass_90_220_hz: 0.30,
            low_mid_220_600_hz: 0.15,
            presence_2k_4k_hz: 0.08,
            air_6k_14k_hz: 0.04,
            spectral_tilt_db: -6.5,
        },
    };

    let p_mono = build_adaptive_dsp_plan(&mono_phone, &sub_heavy_song);
    let p_hp = build_adaptive_dsp_plan(&headphones, &sub_heavy_song);
    let p_bt = build_adaptive_dsp_plan(&bt_a2dp, &sub_heavy_song);

    assert!(p_mono.highpass_hz >= 140.0);
    assert!(p_mono.mono_fold_enabled);
    assert_eq!(p_mono.stereo_width, 0.0);
    assert_eq!(p_mono.crossfeed_strength, 0.0);
    assert!(p_hp.highpass_hz <= 30.0);
    assert!(!p_hp.mono_fold_enabled);
    assert!(p_hp.crossfeed_strength > 0.15);
    assert!(p_bt.ceiling_dbtp <= -1.5);
    assert!(p_bt.crossfeed_strength > 0.0);
    assert!(p_mono.resample_needed);
    assert!(!p_bt.resample_needed);
}

#[test]
fn process_pcm_with_rubato_and_biquad_preserves_sync_and_respects_peak_ceiling() {
    let src = make_sine_stereo(440.0, 44_100, 1.0, 0.9);
    let dev = EffectiveDeviceAcoustics {
        route: AndroidOutputRoute::BluetoothA2dp,
        route_label: "Bluetooth A2DP".to_string(),
        device_name: "Buds".to_string(),
        manufacturer: "Brand".to_string(),
        model: "Phone".to_string(),
        android_api_level: 33,
        effective_sample_rate: 48_000,
        hardware_native_sample_rate: 48_000,
        effective_channel_count: 2,
        is_mono_speaker: false,
        frames_per_burst: 192,
        buffer_capacity_frames: 768,
        low_latency_active: true,
        aaudio_active: true,
        aaudio_device_id: 7,
        aaudio_matched: true,
        estimated_output_latency_ms: 173.0,
        recommended_audio_offset_ms: 173.0,
    };
    let analysis = analyze_song_acoustics(&src, 44_100);
    let plan = build_adaptive_dsp_plan(&dev, &analysis);
    let (out, out_dbtp) = process_pcm_with_plan(&src, &plan).unwrap();

    // Debe conservar duración exacta (~48,000 frames estéreo = 96,000 floats)
    let out_frames = out.len() / 2;
    assert!((out_frames as isize - 48_000).abs() <= 4);
    assert!(out_dbtp <= plan.ceiling_dbtp + 0.05);
}

#[test]
fn lookahead_limiter_pre_attenuates_smoothly_before_transient_and_caps_true_peak() {
    let sr = 48_000_u32;
    let frames = 4_800usize;
    let mut src = Vec::with_capacity(frames * 2);
    // Señal moderada seguida de un transiente fuerte en el frame 2000
    for f in 0..frames {
        let amp = if (2000..2048).contains(&f) { 0.95 } else { 0.20 };
        src.push(amp);
        src.push(-amp);
    }
    let plan = AdaptiveDspPlan {
        profile_name: "Lookahead Test".to_string(),
        adaptation_summary: "Test".to_string(),
        target_lufs: -14.0,
        applied_makeup_gain_db: 6.0, // x2 -> transiente llegaría a 1.90
        ceiling_dbtp: -1.0,          // ~0.89125
        highpass_hz: 24.0,
        mono_fold_enabled: false,
        resample_needed: false,
        source_sample_rate: sr,
        target_sample_rate: sr,
        stereo_width: 1.0,
        crossfeed_strength: 0.0,
        limiter_lookahead_ms: 4.0,
        dither_applied: true,
        high_precision_f32_path: true,
        eq_bands: Vec::new(),
    };
    let (out, out_dbtp) = process_pcm_with_plan(&src, &plan).unwrap();
    assert_eq!(out.len(), frames * 2);
    assert!(out_dbtp <= -0.99);
    // Alrededor de 2 ms antes del transiente (frame 1904..1999) la envolvente ya empieza a reducir ganancia
    let pre_gain_sample = out[1950 * 2].abs();
    let baseline_sample = 0.20 * 10.0_f32.powf(6.0 / 20.0);
    assert!(pre_gain_sample < baseline_sample * 0.96);
}

#[test]
fn adaptive_pipeline_decodes_raw_f32_wav_sources_and_preserves_center_with_crossfeed() {
    let tmp_wav = std::env::temp_dir().join("beatforge_adapt_raw_f32_src.wav");
    let tmp_cache = std::env::temp_dir().join("beatforge_adapt_cache_dir");
    let _ = std::fs::remove_file(&tmp_wav);
    let _ = std::fs::remove_dir_all(&tmp_cache);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut w = hound::WavWriter::create(&tmp_wav, spec).unwrap();
        for i in 0..22_050 {
            let s = ((2.0 * std::f32::consts::PI * 330.0 * (i as f32 / 44_100.0)).sin()
                * 2_000_000.0) as i32;
            w.write_sample(s).unwrap();
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
    }

    let mut configs = AudioTrackSlot::default_three_slots();
    configs[0].file_path = Some(tmp_wav.clone());
    let tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS] = [None, None, None];
    let os_probe = DeviceAudioProbeInput {
        route: AndroidOutputRoute::WiredHeadphones,
        os_sample_rate: 48_000,
        reported_channel_count: 2,
        ..Default::default()
    };
    let aaudio = AudioHardwareProbe {
        aaudio_active: true,
        sample_rate: 48_000,
        channel_count: 2,
        ..Default::default()
    };

    let report = adapt_audio_for_device(
        true,
        &tracks,
        &configs,
        None,
        &os_probe,
        &aaudio,
        &tmp_cache,
        |_, _, _| {},
    )
    .unwrap();

    let plan = report.dsp_plan.expect("dsp plan present");
    assert!(plan.high_precision_f32_path);
    assert!(plan.dither_applied);
    assert!((plan.limiter_lookahead_ms - 4.0).abs() < 0.01);
    assert!((plan.crossfeed_strength - 0.22).abs() < 0.01);
    assert_eq!(plan.source_sample_rate, 44_100);
    assert_eq!(plan.target_sample_rate, 48_000);
    assert!(report.output_true_peak_dbtp <= plan.ceiling_dbtp + 0.05);

    let _ = std::fs::remove_file(&tmp_wav);
    let _ = std::fs::remove_dir_all(&tmp_cache);
}

#[test]
fn streaming_dsp_chunks_match_full_batch_and_bluetooth_latency_is_compensated() {
    let src = make_sine_stereo(220.0, 44_100, 0.8, 0.75);
    let bt_dev = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput {
            route: AndroidOutputRoute::BluetoothA2dp,
            os_sample_rate: 48_000,
            aaudio_matched: true,
            ..Default::default()
        },
        &AudioHardwareProbe {
            aaudio_active: true,
            sample_rate: 48_000,
            frames_per_burst: 192,
            low_latency_active: false,
            device_id: 14,
            ..Default::default()
        },
    );
    assert!(bt_dev.aaudio_matched);
    assert!(bt_dev.recommended_audio_offset_ms >= 140.0);

    let wired_dev = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput {
            route: AndroidOutputRoute::WiredHeadphones,
            aaudio_matched: true,
            ..Default::default()
        },
        &AudioHardwareProbe {
            aaudio_active: true,
            low_latency_active: true,
            device_id: 8,
            ..Default::default()
        },
    );
    assert_eq!(wired_dev.recommended_audio_offset_ms, 0.0);

    let analysis = analyze_song_acoustics(&src, 44_100);
    let plan = build_adaptive_dsp_plan(&bt_dev, &analysis);
    let (batch_out, batch_peak) = process_pcm_with_plan(&src, &plan).unwrap();

    let mut proc = AdaptiveDspProcessor::new(&plan).unwrap();
    let mut streamed_out = Vec::new();
    let mut tmp_chunk = Vec::new();
    for c in src.chunks(1023 * 2) {
        proc.process_chunk(c, &mut tmp_chunk).unwrap();
        streamed_out.extend_from_slice(&tmp_chunk);
    }
    let streamed_peak = proc.finish(&mut tmp_chunk).unwrap();
    streamed_out.extend_from_slice(&tmp_chunk);

    assert_eq!(streamed_out.len(), batch_out.len());
    assert!((streamed_peak - batch_peak).abs() < 1e-4);
    for (a, b) in streamed_out.iter().zip(batch_out.iter()) {
        assert!((a - b).abs() < 1e-5);
    }
}

#[test]
fn cache_hash_ignores_pre_merged_wav_when_raw_sources_exist_and_prune_removes_old_files() {
    let tmp_dir = std::env::temp_dir().join("beatforge_adapt_prune_test");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let raw_src = tmp_dir.join("Inst.wav");
    let merged_a = tmp_dir.join("compiled_mix_a.wav");
    let merged_b = tmp_dir.join("compiled_mix_b.wav");
    std::fs::write(&raw_src, b"RIFF0000WAVE").unwrap();
    std::fs::write(&merged_a, b"RIFFAAAA").unwrap();
    std::fs::write(&merged_b, b"RIFFBBBBBBBB").unwrap();

    let mut configs = AudioTrackSlot::default_three_slots();
    configs[0].file_path = Some(raw_src);
    let tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS] = [None, None, None];
    let dev = fuse_device_and_aaudio_probe(
        &DeviceAudioProbeInput::default(),
        &AudioHardwareProbe::default(),
    );

    let h1 = cache_io::compute_adaptation_cache_hash(&tracks, &configs, Some(&merged_a), &dev);
    let h2 = cache_io::compute_adaptation_cache_hash(&tracks, &configs, Some(&merged_b), &dev);
    let h3 = cache_io::compute_adaptation_cache_hash(&tracks, &configs, None, &dev);
    assert_eq!(h1, h2);
    assert_eq!(h1, h3);

    // Crear 4 entradas en audio_adapted + un .tmp huérfano y un .json huérfano
    let stale_tmp = tmp_dir.join("adapted_999.wav.tmp");
    let orphan_json = tmp_dir.join("adapted_orphan.json");
    let keep_wav = tmp_dir.join("adapted_keep.wav");
    let keep_json = tmp_dir.join("adapted_keep.json");
    std::fs::write(&stale_tmp, b"tmp").unwrap();
    std::fs::write(&orphan_json, b"{}").unwrap();
    std::fs::write(&keep_wav, vec![0u8; 64]).unwrap();
    std::fs::write(&keep_json, b"{}").unwrap();
    for i in 1..=3 {
        std::fs::write(tmp_dir.join(format!("adapted_old{}.wav", i)), vec![0u8; 64]).unwrap();
        std::fs::write(tmp_dir.join(format!("adapted_old{}.json", i)), b"{}").unwrap();
    }

    cache_io::prune_audio_adapted_cache(&tmp_dir, Some(&keep_wav), Some(&keep_json), 2, 1024);
    assert!(!stale_tmp.exists());
    assert!(!orphan_json.exists());
    assert!(keep_wav.exists());
    assert!(keep_json.exists());

    let remaining_wavs = std::fs::read_dir(&tmp_dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("wav"))
        .count();
    assert!(remaining_wavs <= 2);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
