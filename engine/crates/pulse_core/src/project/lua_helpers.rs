//! Archivo: engine/crates/pulse_core/src/project/lua_helpers.rs
//! Propósito: Utilidades internas de escape de cadenas, listas Lua y parseo de parámetros
//! clave-valor / CSV usadas por los generadores de scripts `stage_events.lua` y `ui_blackout.lua`.

/// Escapa comillas, barras invertidas y saltos de línea para emitir un literal de cadena válido en Lua.
pub(super) fn lua_escape_str(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Formatea un slice de cadenas como tabla array de Lua (`{ "a", "b" }`).
pub(super) fn lua_str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "{}".to_string();
    }
    let joined: Vec<String> = items.iter().map(|s| lua_escape_str(s)).collect();
    format!("{{ {} }}", joined.join(", "))
}

/// Divide una lista separada por comas recortando espacios y descartando elementos vacíos.
pub(super) fn split_csv_ids(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Parsea pares `clave=valor` separados por `;` normalizando la clave en minúsculas.
pub(super) fn parse_kv_params(raw: &str) -> Vec<(String, String)> {
    raw.split(';')
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed
                .split_once('=')
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

/// Interpreta banderas booleanas opcionales en español e inglés (`1/true/si/sí/yes/on` vs `0/false/no/off`).
pub(super) fn parse_optional_bool_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "si" | "sí" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
