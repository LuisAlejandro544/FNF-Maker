//! Archivo: engine/crates/pulse_core/src/gfx_backend.rs
//! Propósito: Define los tipos de backend gráfico soportados a nivel de arquitectura
//! (`Canvas2D`, `OpenGL ES 2.0`, `OpenGL ES 3.x` y `Vulkan`) y el reporte de capacidades
//! detectadas en tiempo de ejecución en Android 32 y 64 bits (sin enlazar símbolos C en compile-time).

use serde::{Deserialize, Serialize};

/// Backend de renderizado preferido por el proyecto (preparación modular hacia GPU).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackendKind {
    #[default]
    Canvas2d,
    Gles2,
    Gles3,
    Vulkan,
}

impl RenderBackendKind {
    pub fn as_key(self) -> &'static str {
        match self {
            RenderBackendKind::Canvas2d => "canvas2d",
            RenderBackendKind::Gles2 => "gles2",
            RenderBackendKind::Gles3 => "gles3",
            RenderBackendKind::Vulkan => "vulkan",
        }
    }

    pub fn from_key(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "gles2" | "opengl_es2" | "opengles2" => RenderBackendKind::Gles2,
            "gles3" | "gles" | "opengl_es3" | "opengles3" => RenderBackendKind::Gles3,
            "vulkan" | "vk" => RenderBackendKind::Vulkan,
            _ => RenderBackendKind::Canvas2d,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            RenderBackendKind::Canvas2d => "Canvas 2D (Estable)",
            RenderBackendKind::Gles2 => "OpenGL ES 2.0 (Base)",
            RenderBackendKind::Gles3 => "OpenGL ES 3.x (Base)",
            RenderBackendKind::Vulkan => "Vulkan (Preparado)",
        }
    }
}

/// Capacidades de drivers gráficos detectadas dinámicamente por el motor nativo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GfxCapabilities {
    pub canvas2d_supported: bool,
    pub egl_available: bool,
    pub gles2_supported: bool,
    pub gles3_supported: bool,
    pub vulkan_supported: bool,
    pub active_backend: String,
    pub recommended_backend: String,
    pub detail: String,
}

impl Default for GfxCapabilities {
    fn default() -> Self {
        Self {
            canvas2d_supported: true,
            egl_available: false,
            gles2_supported: false,
            gles3_supported: false,
            vulkan_supported: false,
            active_backend: RenderBackendKind::Canvas2d.as_key().to_string(),
            recommended_backend: RenderBackendKind::Canvas2d.as_key().to_string(),
            detail: "Sondeo gráfico inicializado en modo seguro Canvas 2D.".to_string(),
        }
    }
}
