//! Archivo: engine/crates/pulse_core/src/script_optimizer/scanner.rs
//! Propósito: Utilidades léxicas y análisis de APIs necesarias del motor para detectar
//! llamadas críticas dentro de bucles sin falsos positivos en comentarios o cadenas.

const NECESSARY_ENGINE_APIS: &[&str] = &[
    "setEntityProp",
    "setStrumProp",
    "setCameraZoom",
    "setCameraBaseZoom",
    "setCameraPos",
    "setCameraClamp",
    "shakeCamera",
    "playAnim",
    "playEntityAnim",
    "getSongPosMs",
    "tweenEntityProp",
    "tweenCameraPos",
    "tweenCameraZoom",
    "tweenStrumProp",
    "makeGraphic",
    "makeHudGraphic",
    "makeText",
    "setTextString",
    "cloneSprite",
    "removeSprite",
    "setHealth",
    "addHealth",
    "setScore",
    "addScore",
    "setHudVisible",
    "getHudVisible",
    "setHudOpacity",
    "getHudOpacity",
    "setHudZoom",
    "getHudZoom",
    "tweenHudProp",
    "tweenHudOpacity",
    "setArrowsVisible",
    "getArrowsVisible",
    "setArrowsOpacity",
    "getArrowsOpacity",
    "tweenArrowsOpacity",
    "setScreenBlackout",
    "getScreenBlackout",
    "tweenScreenBlackout",
    "onSongStart",
    "setScrollSpeed",
    "runTimer",
    "setTrackVolume",
    "setTrackMute",
    "flashCamera",
    "flashHud",
    "flashEffect",
    "preloadEntity",
    "retireEntity",
    "swapEntity",
    "setActiveCharacter",
    "setCameraVignette",
    "tweenCameraVignette",
    "clearCameraVignette",
    "getCameraVignetteAlpha",
];

fn strip_comments_for_api_scan(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = String::with_capacity(body.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' || bytes[i] == b'`' {
            let start = i;
            let q = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                    continue;
                }
                if bytes[i] == q {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str(&body[start..i]);
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            if i + 3 < bytes.len() && bytes[i + 2] == b'[' && bytes[i + 3] == b'[' {
                i += 4;
                while i + 1 < bytes.len() && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            } else {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
        } else {
            let ch = body[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Verifica si un bloque contiene al menos una llamada activa a las APIs esenciales del motor.
pub(super) fn contains_necessary_engine_logic(body: &str) -> bool {
    let active_code = strip_comments_for_api_scan(body);
    NECESSARY_ENGINE_APIS.iter().any(|api| active_code.contains(api))
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

pub(super) fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let left_ok = start == 0 || !is_ident_char(bytes[start - 1]);
    let right_ok = end >= bytes.len() || !is_ident_char(bytes[end]);
    left_ok && right_ok
}

pub(super) fn skip_ws_and_js_comments(src: &str, mut idx: usize) -> usize {
    let bytes = src.as_bytes();
    while idx < bytes.len() {
        if bytes[idx].is_ascii_whitespace() {
            idx += 1;
        } else if idx + 1 < bytes.len() && bytes[idx] == b'/' && bytes[idx + 1] == b'/' {
            idx += 2;
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
        } else if idx + 1 < bytes.len() && bytes[idx] == b'/' && bytes[idx + 1] == b'*' {
            idx += 2;
            while idx + 1 < bytes.len() && !(bytes[idx] == b'*' && bytes[idx + 1] == b'/') {
                idx += 1;
            }
            idx = (idx + 2).min(bytes.len());
        } else {
            break;
        }
    }
    idx
}

pub(super) fn find_matching_paren(src: &str, open_idx: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.get(open_idx) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open_idx;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == q {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

pub(super) fn find_matching_brace(src: &str, open_idx: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.get(open_idx) != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open_idx;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == q {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}
