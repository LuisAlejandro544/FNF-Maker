//! Archivo: engine/crates/pulse_android/src/session/gfx_probe.rs
//! Propósito: Sondeo dinámico (en tiempo de ejecución mediante `libc::dlopen`) de drivers nativos
//! disponibles en Android (`libEGL.so`, `libGLESv2.so`, `libGLESv3.so`, `libvulkan.so`) sin
//! enlazar símbolos C en compilación. Prepara la infraestructura hacia renderizado GPU en 32 y 64 bits.

use super::EngineSession;
use pulse_core::gfx_backend::{GfxCapabilities, RenderBackendKind};

#[cfg(target_os = "android")]
fn can_dlopen(soname: &std::ffi::CStr) -> bool {
    // SAFETY: `soname` es una cadena C terminada en NUL válida y estática; cerramos inmediatamente el handle si abre.
    unsafe {
        let handle = libc::dlopen(soname.as_ptr(), libc::RTLD_LAZY);
        if !handle.is_null() {
            libc::dlclose(handle);
            true
        } else {
            false
        }
    }
}

/// Consulta las capacidades gráficas del dispositivo Android para Canvas2D, OpenGL ES 2/3 y Vulkan.
pub fn probe_graphics_capabilities(active_backend: RenderBackendKind) -> GfxCapabilities {
    #[cfg(target_os = "android")]
    let (egl_ok, gles2_ok, gles3_ok, vk_ok) = {
        let egl = can_dlopen(c"libEGL.so");
        let g2 = can_dlopen(c"libGLESv2.so");
        let g3 = can_dlopen(c"libGLESv3.so") || g2;
        let vk = can_dlopen(c"libvulkan.so");
        (egl, g2, g3, vk)
    };

    #[cfg(not(target_os = "android"))]
    let (egl_ok, gles2_ok, gles3_ok, vk_ok) = (false, false, false, false);

    let recommended = if vk_ok {
        RenderBackendKind::Vulkan
    } else if gles3_ok {
        RenderBackendKind::Gles3
    } else if gles2_ok {
        RenderBackendKind::Gles2
    } else {
        RenderBackendKind::Canvas2d
    };

    let detail = format!(
        "EGL: {} | GLES2: {} | GLES3: {} | Vulkan: {} (Renderer actual: {})",
        if egl_ok { "Sí" } else { "No" },
        if gles2_ok { "Sí" } else { "No" },
        if gles3_ok { "Sí" } else { "No" },
        if vk_ok { "Sí" } else { "No" },
        active_backend.label_es()
    );

    GfxCapabilities {
        canvas2d_supported: true,
        egl_available: egl_ok,
        gles2_supported: gles2_ok,
        gles3_supported: gles3_ok,
        vulkan_supported: vk_ok,
        active_backend: active_backend.as_key().to_string(),
        recommended_backend: recommended.as_key().to_string(),
        detail,
    }
}

impl EngineSession {
    pub fn probe_graphics_backends_json(&self) -> String {
        let caps = probe_graphics_capabilities(self.project.preferred_render_backend);
        serde_json::to_string(&caps).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn set_preferred_render_backend(&mut self, backend_key: &str) -> String {
        let parsed = RenderBackendKind::from_key(backend_key);
        self.project.preferred_render_backend = parsed;
        self.save_active_project_now();
        self.ok_response(format!("Backend gráfico preparado: {}", parsed.label_es()))
    }
}
