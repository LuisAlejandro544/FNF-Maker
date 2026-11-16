//! Archivo: engine/crates/pulse_audio/src/rust_audio_backend.rs
//! Propósito: Backend de audio de ultra-baja latencia 100% en Rust puro (cero C++, sin `oboe-sys`).
//! En Android (API 26+ / Android 8.0+), carga dinámicamente `libaaudio.so` mediante `dlopen`/`dlsym`
//! usando exclusivamente el C ABI estable de AAudio en modo `LOW_LATENCY` con callback en tiempo real.
//! En dispositivos Android 7.0/7.1 (API 24-25 sin AAudio) o entornos de escritorio sin hardware de audio,
//! mantiene un reloj de respaldo en Rust puro para que el juego siga sincronizado sin crashear.

use crate::mixer::AudioMixerState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Manejador vivo del stream de audio en Rust puro. Mientras permanezca en memoria, el audio fluye.
/// Se inicializa de forma diferida (`new_deferred` + `ensure_started`) para evitar bloqueos o fallos
/// OEM durante el arranque de la Activity Android.
pub struct AudioEngineHandle {
    pub mixer: Arc<AudioMixerState>,
    #[cfg(target_os = "android")]
    stream: Option<AndroidAudioOutput>,
    #[cfg(target_os = "android")]
    start_attempted: bool,
    #[cfg(target_os = "android")]
    needs_restart: Arc<AtomicBool>,
}

impl AudioEngineHandle {
    /// Crea el estado del mezclador sin abrir todavía hardware de audio Android.
    pub fn new_deferred() -> Self {
        let default_sr = 48_000_u32;
        let mixer = AudioMixerState::new(default_sr);
        Self {
            mixer,
            #[cfg(target_os = "android")]
            stream: None,
            #[cfg(target_os = "android")]
            start_attempted: false,
            #[cfg(target_os = "android")]
            needs_restart: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Inicializa inmediatamente el motor de audio (útil en pruebas o invocación directa).
    pub fn start_default() -> Self {
        let mut handle = Self::new_deferred();
        handle.ensure_started();
        handle
    }

    /// Abre el stream nativo AAudio en Rust puro bajo demanda (idempotente) o activa el reloj
    /// de respaldo seguro si el dispositivo es Android 7.x o rechaza el stream de baja latencia.
    pub fn ensure_started(&mut self) {
        #[cfg(target_os = "android")]
        {
            if self.needs_restart.swap(false, Ordering::AcqRel) {
                self.stream = None;
                self.start_attempted = false;
            }

            if self.stream.is_some() || self.start_attempted {
                return;
            }
            self.start_attempted = true;

            match android_aaudio::try_start_aaudio(
                Arc::clone(&self.mixer),
                Arc::clone(&self.needs_restart),
            ) {
                Ok((stream, actual_sr)) => {
                    if actual_sr > 0 {
                        self.mixer.sample_rate.store(actual_sr, Ordering::Relaxed);
                    }
                    log::info!(
                        "Stream AAudio (100% Rust) iniciado correctamente a {} Hz",
                        actual_sr
                    );
                    self.stream = Some(AndroidAudioOutput::AAudio(stream));
                }
                Err(err) => {
                    log::warn!(
                        "AAudio no disponible ({}); activando reloj de audio Rust de respaldo.",
                        err
                    );
                    let fallback = FallbackClockThread::start(Arc::clone(&self.mixer));
                    self.stream = Some(AndroidAudioOutput::FallbackClock(fallback));
                }
            }
        }
    }
}

#[cfg(target_os = "android")]
enum AndroidAudioOutput {
    AAudio(android_aaudio::AAudioStreamHandle),
    FallbackClock(FallbackClockThread),
}

/// Reloj de respaldo en Rust puro para Android 7.0/7.1 (API 24-25) o emuladores sin AAudio.
#[cfg(target_os = "android")]
struct FallbackClockThread {
    stop_flag: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "android")]
impl FallbackClockThread {
    fn start(mixer: Arc<AudioMixerState>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);
        let handle = std::thread::Builder::new()
            .name("pulse_audio_fallback_clock".to_string())
            .spawn(move || {
                let mut last = std::time::Instant::now();
                let mut scratch = vec![0.0_f32; 512 * 2];
                while !stop_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last).as_secs_f64();
                    last = now;
                    if mixer.is_playing.load(Ordering::Acquire) {
                        let sr = mixer.sample_rate.load(Ordering::Relaxed).max(1) as f64;
                        let frames = ((elapsed * sr).round() as usize).clamp(1, 4096);
                        let needed = frames * 2;
                        if scratch.len() < needed {
                            scratch.resize(needed, 0.0);
                        }
                        mixer.render_audio_frames(&mut scratch[..needed]);
                    }
                }
            })
            .ok();

        Self { stop_flag, handle }
    }
}

#[cfg(target_os = "android")]
impl Drop for FallbackClockThread {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Enlace FFI en Rust puro al C ABI de `libaaudio.so` sin C++ ni `oboe-sys`.
#[cfg(target_os = "android")]
mod android_aaudio {
    use crate::mixer::AudioMixerState;
    use std::ffi::{CStr, c_void};
    use std::ptr::null_mut;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const AAUDIO_OK: i32 = 0;
    const AAUDIO_DIRECTION_OUTPUT: i32 = 0;
    const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
    const AAUDIO_SHARING_MODE_SHARED: i32 = 1;
    const AAUDIO_PERFORMANCE_MODE_NONE: i32 = 10;
    const AAUDIO_PERFORMANCE_MODE_LOW_LATENCY: i32 = 12;
    const AAUDIO_CALLBACK_RESULT_CONTINUE: i32 = 0;

    #[repr(C)]
    struct AAudioStreamBuilder {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct AAudioStream {
        _private: [u8; 0],
    }

    type FnCreateStreamBuilder = unsafe extern "C" fn(*mut *mut AAudioStreamBuilder) -> i32;
    type FnBuilderSetI32 = unsafe extern "C" fn(*mut AAudioStreamBuilder, i32);
    type FnDataCallback = unsafe extern "C" fn(
        stream: *mut AAudioStream,
        user_data: *mut c_void,
        audio_data: *mut c_void,
        num_frames: i32,
    ) -> i32;
    type FnErrorCallback =
        unsafe extern "C" fn(stream: *mut AAudioStream, user_data: *mut c_void, error: i32);
    type FnBuilderSetDataCb =
        unsafe extern "C" fn(*mut AAudioStreamBuilder, FnDataCallback, *mut c_void);
    type FnBuilderSetErrorCb =
        unsafe extern "C" fn(*mut AAudioStreamBuilder, FnErrorCallback, *mut c_void);
    type FnBuilderOpenStream =
        unsafe extern "C" fn(*mut AAudioStreamBuilder, *mut *mut AAudioStream) -> i32;
    type FnBuilderDelete = unsafe extern "C" fn(*mut AAudioStreamBuilder) -> i32;
    type FnStreamGetSampleRate = unsafe extern "C" fn(*mut AAudioStream) -> i32;
    type FnStreamCmd = unsafe extern "C" fn(*mut AAudioStream) -> i32;

    struct AAudioApi {
        _lib: *mut c_void,
        create_stream_builder: FnCreateStreamBuilder,
        set_direction: FnBuilderSetI32,
        set_performance_mode: FnBuilderSetI32,
        set_sharing_mode: FnBuilderSetI32,
        set_format: FnBuilderSetI32,
        set_channel_count: FnBuilderSetI32,
        set_sample_rate: FnBuilderSetI32,
        set_data_callback: FnBuilderSetDataCb,
        set_error_callback: FnBuilderSetErrorCb,
        open_stream: FnBuilderOpenStream,
        delete_builder: FnBuilderDelete,
        get_sample_rate: FnStreamGetSampleRate,
        request_start: FnStreamCmd,
        request_stop: FnStreamCmd,
        close_stream: FnStreamCmd,
    }

    unsafe fn load_sym<T>(lib: *mut c_void, name: &CStr) -> Result<T, String> {
        let ptr = unsafe { libc::dlsym(lib, name.as_ptr()) };
        if ptr.is_null() {
            return Err(format!("Símbolo AAudio faltante: {:?}", name));
        }
        Ok(unsafe { std::mem::transmute_copy(&ptr) })
    }

    impl AAudioApi {
        fn load() -> Result<Self, String> {
            let lib = unsafe { libc::dlopen(c"libaaudio.so".as_ptr(), libc::RTLD_NOW) };
            if lib.is_null() {
                return Err("libaaudio.so no presente (posible Android < 8.0)".to_string());
            }

            unsafe {
                let set_channel_count = load_sym::<FnBuilderSetI32>(
                    lib,
                    c"AAudioStreamBuilder_setChannelCount",
                )
                .or_else(|_| {
                    load_sym::<FnBuilderSetI32>(lib, c"AAudioStreamBuilder_setSamplesPerFrame")
                })?;

                Ok(Self {
                    _lib: lib,
                    create_stream_builder: load_sym(lib, c"AAudio_createStreamBuilder")?,
                    set_direction: load_sym(lib, c"AAudioStreamBuilder_setDirection")?,
                    set_performance_mode: load_sym(lib, c"AAudioStreamBuilder_setPerformanceMode")?,
                    set_sharing_mode: load_sym(lib, c"AAudioStreamBuilder_setSharingMode")?,
                    set_format: load_sym(lib, c"AAudioStreamBuilder_setFormat")?,
                    set_channel_count,
                    set_sample_rate: load_sym(lib, c"AAudioStreamBuilder_setSampleRate")?,
                    set_data_callback: load_sym(lib, c"AAudioStreamBuilder_setDataCallback")?,
                    set_error_callback: load_sym(lib, c"AAudioStreamBuilder_setErrorCallback")?,
                    open_stream: load_sym(lib, c"AAudioStreamBuilder_openStream")?,
                    delete_builder: load_sym(lib, c"AAudioStreamBuilder_delete")?,
                    get_sample_rate: load_sym(lib, c"AAudioStream_getSampleRate")?,
                    request_start: load_sym(lib, c"AAudioStream_requestStart")?,
                    request_stop: load_sym(lib, c"AAudioStream_requestStop")?,
                    close_stream: load_sym(lib, c"AAudioStream_close")?,
                })
            }
        }
    }

    struct CallbackContext {
        mixer: Arc<AudioMixerState>,
        needs_restart: Arc<AtomicBool>,
    }

    pub struct AAudioStreamHandle {
        stream: *mut AAudioStream,
        request_stop: FnStreamCmd,
        close_stream: FnStreamCmd,
        ctx_ptr: *mut CallbackContext,
    }

    unsafe impl Send for AAudioStreamHandle {}
    unsafe impl Sync for AAudioStreamHandle {}

    impl Drop for AAudioStreamHandle {
        fn drop(&mut self) {
            unsafe {
                // Detener y cerrar el stream antes de liberar CallbackContext garantiza que ningún
                // callback en tiempo real vuelva a tocar ctx_ptr.
                let _ = (self.request_stop)(self.stream);
                let _ = (self.close_stream)(self.stream);
                drop(Box::from_raw(self.ctx_ptr));
            }
        }
    }

    unsafe extern "C" fn aaudio_data_callback(
        _stream: *mut AAudioStream,
        user_data: *mut c_void,
        audio_data: *mut c_void,
        num_frames: i32,
    ) -> i32 {
        if user_data.is_null() || audio_data.is_null() || num_frames <= 0 {
            return AAUDIO_CALLBACK_RESULT_CONTINUE;
        }
        let ctx = unsafe { &*(user_data as *const CallbackContext) };
        let len_floats = (num_frames as usize) * 2;
        let interleaved = unsafe { std::slice::from_raw_parts_mut(audio_data as *mut f32, len_floats) };
        ctx.mixer.render_audio_frames(interleaved);
        AAUDIO_CALLBACK_RESULT_CONTINUE
    }

    unsafe extern "C" fn aaudio_error_callback(
        _stream: *mut AAudioStream,
        user_data: *mut c_void,
        error: i32,
    ) {
        if !user_data.is_null() {
            let ctx = unsafe { &*(user_data as *const CallbackContext) };
            ctx.needs_restart.store(true, Ordering::Release);
        }
        log::warn!("AAudio reportó evento/desconexión de stream: código {}", error);
    }

    pub fn try_start_aaudio(
        mixer: Arc<AudioMixerState>,
        needs_restart: Arc<AtomicBool>,
    ) -> Result<(AAudioStreamHandle, u32), String> {
        let api = AAudioApi::load()?;

        let ctx_ptr = Box::into_raw(Box::new(CallbackContext {
            mixer,
            needs_restart,
        }));

        let try_open_profile = |low_latency: bool, sample_rate: i32| -> Result<*mut AAudioStream, i32> {
            let mut builder: *mut AAudioStreamBuilder = null_mut();
            let res = unsafe { (api.create_stream_builder)(&mut builder) };
            if res != AAUDIO_OK || builder.is_null() {
                return Err(res);
            }

            unsafe {
                (api.set_direction)(builder, AAUDIO_DIRECTION_OUTPUT);
                (api.set_performance_mode)(
                    builder,
                    if low_latency {
                        AAUDIO_PERFORMANCE_MODE_LOW_LATENCY
                    } else {
                        AAUDIO_PERFORMANCE_MODE_NONE
                    },
                );
                (api.set_sharing_mode)(builder, AAUDIO_SHARING_MODE_SHARED);
                (api.set_format)(builder, AAUDIO_FORMAT_PCM_FLOAT);
                (api.set_channel_count)(builder, 2);
                if sample_rate > 0 {
                    (api.set_sample_rate)(builder, sample_rate);
                }
                (api.set_data_callback)(builder, aaudio_data_callback, ctx_ptr.cast());
                (api.set_error_callback)(builder, aaudio_error_callback, ctx_ptr.cast());

                let mut stream: *mut AAudioStream = null_mut();
                let open_res = (api.open_stream)(builder, &mut stream);
                let _ = (api.delete_builder)(builder);

                if open_res != AAUDIO_OK || stream.is_null() {
                    return Err(open_res);
                }
                Ok(stream)
            }
        };

        let stream = match try_open_profile(true, 48_000) {
            Ok(s) => s,
            Err(err_ll) => {
                log::warn!(
                    "AAudio LowLatency 48kHz falló ({}); reintentando perfil estándar...",
                    err_ll
                );
                match try_open_profile(false, 0) {
                    Ok(s) => s,
                    Err(err_safe) => {
                        unsafe {
                            drop(Box::from_raw(ctx_ptr));
                        }
                        return Err(format!("openStream falló (código {})", err_safe));
                    }
                }
            }
        };

        let actual_sr = unsafe { (api.get_sample_rate)(stream) }.max(0) as u32;
        let start_res = unsafe { (api.request_start)(stream) };
        if start_res != AAUDIO_OK {
            unsafe {
                let _ = (api.close_stream)(stream);
                drop(Box::from_raw(ctx_ptr));
            }
            return Err(format!("requestStart falló (código {})", start_res));
        }

        Ok((
            AAudioStreamHandle {
                stream,
                request_stop: api.request_stop,
                close_stream: api.close_stream,
                ctx_ptr,
            },
            actual_sr,
        ))
    }
}
