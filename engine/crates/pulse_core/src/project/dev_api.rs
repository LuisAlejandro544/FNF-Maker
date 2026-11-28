//! Archivo: engine/crates/pulse_core/src/project/dev_api.rs
//! Propósito: Sistema centralizado de versionado "API de desarrollo" (`CURRENT_DEV_API_VERSION`).
//! Permite identificar qué capacidades del motor, editor de chart, multicámara, scripting y
//! cuenta regresiva adopta automáticamente cada proyecto nuevo, y migrar proyectos previos de forma explícita.

use super::models::ModProject;

/// Versión por defecto asignada a proyectos antiguos sin campo `dev_api_version`.
pub const LEGACY_DEV_API_VERSION: u32 = 1;

/// Versión vigente de la API de desarrollo de BeatForge Studio.
/// Incrementar este valor al introducir nuevas capacidades de editor/chart/motor.
pub const CURRENT_DEV_API_VERSION: u32 = 2;

/// Descriptor de una versión específica de la API de desarrollo.
#[derive(Debug, Clone, Copy)]
pub struct DevApiSpec {
    pub version: u32,
    pub codename: &'static str,
    pub summary: &'static str,
    pub features: &'static [&'static str],
}

/// Etiqueta compacta visible en UI (ej. "v2 · Scripting Pro & Multicámara").
pub fn dev_api_label(version: u32) -> String {
    let spec = dev_api_spec(version);
    format!("v{} · {}", spec.version, spec.codename)
}

/// Devuelve la especificación documentada de cada versión de la API de desarrollo.
pub fn dev_api_spec(version: u32) -> DevApiSpec {
    match version {
        0 | 1 => DevApiSpec {
            version: 1,
            codename: "Base Studio",
            summary: "Estructura inicial de proyecto, chart clásico y scripting Lua/TS/JS básico.",
            features: &[
                "Chart JSON estándar y pistas de audio (máx. 3)",
                "Escenario 1280x720 con cámara única",
                "Scripts Lua 5.4, TypeScript (oxc) y JavaScript",
            ],
        },
        _ => DevApiSpec {
            version: CURRENT_DEV_API_VERSION,
            codename: "Scripting Pro & Multicámara",
            summary: "Editor con líneas y Asistente IA, multicámara, timeline cues, strums y cuenta 2-1-¡YA!.",
            features: &[
                "Multicámara (Cam 0..3+) y viñeta lateral animable por Lua/Chart",
                "Timeline cues (stage_events.lua, ui_blackout.lua) y precarga RAM de entidades",
                "Modcharts de strums, gráficos/textos HUD dinámicos y notas especiales (X/Esquive/Alt)",
                "Cuenta regresiva sincronizada de 3 pasos (2 · 1 · ¡YA!)",
                "Editor de scripts con numeración de líneas y Asistente IA con Skills .md",
            ],
        },
    }
}

/// Migra explícitamente un `ModProject` hacia `target_version`, garantizando scripts del sistema
/// y estructuras actualizadas sin perder contenido existente del usuario.
pub fn migrate_project_to_dev_api(proj: &mut ModProject, target_version: u32) -> (u32, u32) {
    let previous = proj.dev_api_version.max(LEGACY_DEV_API_VERSION);
    let clamped_target = target_version.clamp(LEGACY_DEV_API_VERSION, CURRENT_DEV_API_VERSION);
    proj.stage.camera.ensure_default_cameras();
    proj.countdown.sanitize();
    proj.ensure_camera_script();
    proj.ensure_stage_events_script();
    proj.sync_ui_blackout_script();
    proj.dev_api_version = clamped_target;
    (previous, clamped_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_projects_default_to_current_dev_api_version() {
        let demo = ModProject::new_demo();
        assert_eq!(demo.dev_api_version, CURRENT_DEV_API_VERSION);

        let blank = ModProject::new_blank("p_test", "Test", "Autor");
        assert_eq!(blank.dev_api_version, CURRENT_DEV_API_VERSION);
    }

    #[test]
    fn legacy_json_without_dev_api_defaults_to_v1_and_migrates_cleanly() {
        let mut proj = ModProject::new_demo();
        proj.dev_api_version = LEGACY_DEV_API_VERSION;
        let (prev, next) = migrate_project_to_dev_api(&mut proj, CURRENT_DEV_API_VERSION);
        assert_eq!(prev, 1);
        assert_eq!(next, CURRENT_DEV_API_VERSION);
        assert_eq!(proj.dev_api_version, CURRENT_DEV_API_VERSION);
    }
}
