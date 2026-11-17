//! Archivo: engine/crates/pulse_core/src/script_optimizer.rs
//! Propósito: Analizador y optimizador estático inteligente para scripts Lua 5.4, JavaScript y TypeScript
//! ejecutado durante la compilación en caliente (`hot_compile`).
//! Detecta bucles infinitos o de alto consumo de CPU (especialmente dentro de `onUpdate`):
//! - Si un bucle pesado no contiene lógica necesaria del motor (`setEntityProp`, `setCameraZoom`,
//!   `shakeCamera`, `playAnim`, `getSongPosMs`), descarta el bucle por completo.
//! - Si un bucle pesado/infinito sí contiene lógica necesaria del motor, descarta la repetición
//!   peligrosa (reemplaza bucles infinitos por ejecución única segura o limita iteraciones excesivas)
//!   para proteger la fluidez en teléfonos Android de 32 y 64 bits sin romper la sintaxis.

use crate::project::{ScriptAsset, ScriptLanguage};

/// Resultado del análisis y optimización inteligente sobre un script individual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLoopOptResult {
    pub optimized_source: String,
    pub discarded_loops: usize,
    pub neutralized_loops: usize,
}

/// Reporte acumulado de optimización de scripts durante `hot_compile`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptBatchReport {
    pub analyzed_scripts: usize,
    pub modified_scripts: usize,
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
}

const NECESSARY_ENGINE_APIS: &[&str] = &[
    "setEntityProp",
    "setStrumProp",
    "setCameraZoom",
    "setCameraBaseZoom",
    "setCameraPos",
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
    "setHudOpacity",
    "setHudZoom",
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

fn contains_necessary_engine_logic(body: &str) -> bool {
    let active_code = strip_comments_for_api_scan(body);
    NECESSARY_ENGINE_APIS.iter().any(|api| active_code.contains(api))
}

/// Optimiza todos los scripts habilitados y devuelve las copias listas para ejecutarse en gameplay.
pub fn optimize_scripts_for_hot_compile(
    scripts: &[ScriptAsset],
) -> (Vec<ScriptAsset>, ScriptBatchReport) {
    let mut out = Vec::with_capacity(scripts.len());
    let mut report = ScriptBatchReport::default();

    for script in scripts {
        if !script.enabled {
            out.push(script.clone());
            continue;
        }
        report.analyzed_scripts += 1;
        let res = optimize_single_script(script.language, &script.source_code);
        if res.discarded_loops > 0 || res.neutralized_loops > 0 {
            report.modified_scripts += 1;
            report.discarded_heavy_loops += res.discarded_loops;
            report.neutralized_unbounded_loops += res.neutralized_loops;
        }
        let mut updated = script.clone();
        updated.source_code = res.optimized_source;
        out.push(updated);
    }

    (out, report)
}

/// Analiza y optimiza bucles pesados/infinitos de un script según su lenguaje.
pub fn optimize_single_script(language: ScriptLanguage, source: &str) -> ScriptLoopOptResult {
    match language {
        ScriptLanguage::Lua => optimize_lua_loops(source),
        ScriptLanguage::JavaScript | ScriptLanguage::TypeScript => optimize_js_ts_loops(source),
    }
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let left_ok = start == 0 || !is_ident_char(bytes[start - 1]);
    let right_ok = end >= bytes.len() || !is_ident_char(bytes[end]);
    left_ok && right_ok
}

fn skip_ws_and_js_comments(src: &str, mut idx: usize) -> usize {
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

fn find_matching_paren(src: &str, open_idx: usize) -> Option<usize> {
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

fn find_matching_brace(src: &str, open_idx: usize) -> Option<usize> {
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

fn is_unbounded_js_condition(cond: &str) -> bool {
    let compact: String = cond.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(compact.as_str(), "true" | "1" | "!0" | "!false")
}

fn is_unbounded_js_for_header(header: &str) -> bool {
    let parts: Vec<&str> = header.split(';').collect();
    if parts.len() != 3 {
        return false;
    }
    let cond: String = parts[1].chars().filter(|c| !c.is_whitespace()).collect();
    cond.is_empty() || is_unbounded_js_condition(&cond)
}

fn extract_simple_numeric_bound(cond: &str) -> Option<(usize, usize, u64)> {
    let op_idx = cond.find('<').or_else(|| cond.find('>'))?;
    let after_op_start = if cond.as_bytes().get(op_idx + 1) == Some(&b'=') {
        op_idx + 2
    } else {
        op_idx + 1
    };
    let rhs = &cond[after_op_start..];
    let trimmed = rhs.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return None;
    }
    let lead_ws = rhs.len() - rhs.trim_start().len();
    let num_start = after_op_start + lead_ws;
    let num_end = num_start + trimmed.len();
    let clean: String = trimmed.chars().filter(|c| *c != '_').collect();
    let val = clean.parse::<u64>().ok()?;
    Some((num_start, num_end, val))
}

fn estimate_js_for_iterations(header: &str) -> Option<u64> {
    let parts: Vec<&str> = header.split(';').collect();
    if parts.len() != 3 {
        return None;
    }
    extract_simple_numeric_bound(parts[1]).map(|(_, _, v)| v)
}

fn cap_js_for_header(header: &str, cap: u64) -> Option<String> {
    let parts: Vec<&str> = header.split(';').collect();
    if parts.len() != 3 {
        return None;
    }
    let cond = parts[1];
    if let Some((start, end, val)) = extract_simple_numeric_bound(cond) {
        if val > cap {
            let mut new_cond = String::with_capacity(cond.len());
            new_cond.push_str(&cond[..start]);
            new_cond.push_str(&cap.to_string());
            new_cond.push_str(&cond[end..]);
            return Some(format!("{};{};{}", parts[0], new_cond, parts[2]));
        }
    }
    None
}

fn is_inside_on_update(src: &str, pos: usize) -> bool {
    let prefix = &src[..pos];
    if let Some(last_fn) = prefix.rfind("onUpdate") {
        let between = &prefix[last_fn..];
        let b = between.as_bytes();
        let mut i = 0usize;
        let mut opens = 0usize;
        let mut closes = 0usize;
        while i < b.len() {
            match b[i] {
                b'\'' | b'"' | b'`' => {
                    let q = b[i];
                    i += 1;
                    while i < b.len() {
                        if b[i] == b'\\' {
                            i = (i + 2).min(b.len());
                            continue;
                        }
                        if b[i] == q {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                }
                b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                    i += 2;
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                    i += 2;
                    while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(b.len());
                }
                b'{' => {
                    opens += 1;
                    i += 1;
                }
                b'}' => {
                    closes += 1;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        return opens > closes;
    }
    false
}

fn optimize_js_ts_loops(source: &str) -> ScriptLoopOptResult {
    let mut output = String::with_capacity(source.len());
    let mut discarded_loops = 0usize;
    let mut neutralized_loops = 0usize;

    let bytes = source.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        // Preservar strings y comentarios sin tocar nada dentro
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
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
                output.push_str(&source[start..i]);
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                output.push_str(&source[start..i]);
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                let start = i;
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                output.push_str(&source[start..i]);
                continue;
            }
            _ => {}
        }

        let is_while = source[i..].starts_with("while") && is_word_boundary(bytes, i, i + 5);
        let is_for = source[i..].starts_with("for") && is_word_boundary(bytes, i, i + 3);

        if is_while || is_for {
            let kw_len = if is_while { 5 } else { 3 };
            let after_kw = skip_ws_and_js_comments(source, i + kw_len);
            if bytes.get(after_kw) == Some(&b'(') {
                if let Some(close_paren) = find_matching_paren(source, after_kw) {
                    let header = &source[after_kw + 1..close_paren];
                    let after_paren = skip_ws_and_js_comments(source, close_paren + 1);
                    if bytes.get(after_paren) == Some(&b'{') {
                        if let Some(close_brace) = find_matching_brace(source, after_paren) {
                            let body = &source[after_paren + 1..close_brace];
                            let in_update = is_inside_on_update(source, i);
                            let safe_cap = if in_update { 64 } else { 256 };

                            let unbounded = if is_while {
                                is_unbounded_js_condition(header) && !body.contains("break")
                            } else {
                                is_unbounded_js_for_header(header) && !body.contains("break")
                            };

                            let est_iters = if is_for {
                                estimate_js_for_iterations(header)
                            } else {
                                None
                            };

                            let is_heavy = unbounded || est_iters.map(|n| n > safe_cap).unwrap_or(false);
                            if is_heavy {
                                if !contains_necessary_engine_logic(body) {
                                    output.push_str("/* [HotCompile] Bucle pesado sin lógica necesaria descartado */");
                                    discarded_loops += 1;
                                    i = close_brace + 1;
                                    continue;
                                } else if unbounded {
                                    // Descarta el bucle infinito pero conserva la lógica necesaria en 1 sola pasada segura
                                    output.push_str("/* [HotCompile] Bucle infinito neutralizado */ for (let _bf_once = 0; _bf_once < 1; _bf_once++) {");
                                    let inner_opt = optimize_js_ts_loops(body);
                                    discarded_loops += inner_opt.discarded_loops;
                                    neutralized_loops += inner_opt.neutralized_loops + 1;
                                    output.push_str(&inner_opt.optimized_source);
                                    output.push('}');
                                    i = close_brace + 1;
                                    continue;
                                } else if let Some(capped_header) = cap_js_for_header(header, safe_cap) {
                                    output.push_str("for (");
                                    output.push_str(&capped_header);
                                    output.push_str(") {");
                                    let inner_opt = optimize_js_ts_loops(body);
                                    discarded_loops += inner_opt.discarded_loops;
                                    neutralized_loops += inner_opt.neutralized_loops + 1;
                                    output.push_str(&inner_opt.optimized_source);
                                    output.push('}');
                                    i = close_brace + 1;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }

        let ch = source[i..].chars().next().unwrap();
        output.push(ch);
        i += ch.len_utf8();
    }

    ScriptLoopOptResult {
        optimized_source: output,
        discarded_loops,
        neutralized_loops,
    }
}

fn is_lua_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_lua_word(bytes: &[u8], start: usize, word: &str) -> bool {
    let end = start + word.len();
    if end > bytes.len() {
        return false;
    }
    if &bytes[start..end] != word.as_bytes() {
        return false;
    }
    let left_ok = start == 0 || !is_lua_ident_char(bytes[start - 1]);
    let right_ok = end == bytes.len() || !is_lua_ident_char(bytes[end]);
    left_ok && right_ok
}

fn find_lua_loop_end(source: &str, body_start: usize, is_repeat: bool) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut i = body_start;
    let mut depth = 1usize;
    let mut expecting_loop_do = false;

    while i < bytes.len() {
        // Saltar comentarios Lua (`--` y `--[[ ... ]]`)
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
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
            continue;
        }
        // Saltar strings Lua
        if bytes[i] == b'\'' || bytes[i] == b'"' {
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
            continue;
        }

        if is_lua_word(bytes, i, "elseif") {
            i += 6;
            continue;
        } else if is_lua_word(bytes, i, "function") {
            depth += 1;
            i += 8;
            continue;
        } else if is_lua_word(bytes, i, "if") {
            depth += 1;
            i += 2;
            continue;
        } else if is_lua_word(bytes, i, "for") {
            depth += 1;
            expecting_loop_do = true;
            i += 3;
            continue;
        } else if is_lua_word(bytes, i, "while") {
            depth += 1;
            expecting_loop_do = true;
            i += 5;
            continue;
        } else if is_lua_word(bytes, i, "repeat") {
            depth += 1;
            i += 6;
            continue;
        } else if is_lua_word(bytes, i, "do") {
            if expecting_loop_do {
                expecting_loop_do = false;
            } else {
                depth += 1;
            }
            i += 2;
            continue;
        } else if is_lua_word(bytes, i, "end") {
            depth -= 1;
            if depth == 0 && !is_repeat {
                return Some((i, i + 3));
            }
            i += 3;
            continue;
        } else if is_lua_word(bytes, i, "until") {
            depth -= 1;
            if depth == 0 && is_repeat {
                let mut cond_end = i + 5;
                while cond_end < bytes.len() && bytes[cond_end] != b'\n' {
                    cond_end += 1;
                }
                return Some((i, cond_end));
            }
            i += 5;
            continue;
        }

        i += 1;
    }
    None
}

fn find_lua_do_keyword(source: &str, start: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
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
            continue;
        }
        if bytes[i] == b'\'' || bytes[i] == b'"' {
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
            continue;
        }
        if is_lua_word(bytes, i, "do") {
            return Some((i, i + 2));
        }
        if is_lua_word(bytes, i, "end") || is_lua_word(bytes, i, "function") {
            return None;
        }
        i += 1;
    }
    None
}

fn is_lua_inside_on_update(src: &str, pos: usize) -> bool {
    let prefix = &src[..pos];
    if let Some(idx) = prefix.rfind("onUpdate") {
        let snippet = &prefix[idx..];
        let b = snippet.as_bytes();
        let mut depth: i32 = 1;
        let mut k = 0usize;
        let mut exp_do = false;
        while k < b.len() {
            if k + 1 < b.len() && b[k] == b'-' && b[k + 1] == b'-' {
                if k + 3 < b.len() && b[k + 2] == b'[' && b[k + 3] == b'[' {
                    k += 4;
                    while k + 1 < b.len() && !(b[k] == b']' && b[k + 1] == b']') {
                        k += 1;
                    }
                    k = (k + 2).min(b.len());
                } else {
                    k += 2;
                    while k < b.len() && b[k] != b'\n' {
                        k += 1;
                    }
                }
                continue;
            }
            if b[k] == b'\'' || b[k] == b'"' {
                let q = b[k];
                k += 1;
                while k < b.len() {
                    if b[k] == b'\\' {
                        k = (k + 2).min(b.len());
                        continue;
                    }
                    if b[k] == q {
                        k += 1;
                        break;
                    }
                    k += 1;
                }
                continue;
            }
            if is_lua_word(b, k, "elseif") {
                k += 6;
            } else if is_lua_word(b, k, "if") {
                depth += 1;
                k += 2;
            } else if is_lua_word(b, k, "for") {
                depth += 1;
                exp_do = true;
                k += 3;
            } else if is_lua_word(b, k, "while") {
                depth += 1;
                exp_do = true;
                k += 5;
            } else if is_lua_word(b, k, "do") {
                if exp_do {
                    exp_do = false;
                } else {
                    depth += 1;
                }
                k += 2;
            } else if is_lua_word(b, k, "function") && k > 8 {
                depth += 1;
                k += 8;
            } else if is_lua_word(b, k, "end") {
                depth -= 1;
                k += 3;
            } else {
                k += 1;
            }
        }
        return depth > 0;
    }
    false
}

fn parse_lua_numeric_for_limit(header: &str) -> Option<(usize, usize, u64)> {
    // Ej: `i = 1, 100000` o `i = 0, 50000, 1`
    let eq_pos = header.find('=')?;
    let after_eq = &header[eq_pos + 1..];
    let parts: Vec<&str> = after_eq.split(',').collect();
    if parts.len() < 2 {
        return None;
    }
    let limit_part = parts[1];
    let trimmed = limit_part.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return None;
    }
    let lead_ws = limit_part.len() - limit_part.trim_start().len();
    let offset_in_header = eq_pos + 1 + parts[0].len() + 1;
    let num_start = offset_in_header + lead_ws;
    let num_end = num_start + trimmed.len();
    let clean: String = trimmed.chars().filter(|c| *c != '_').collect();
    let val = clean.parse::<u64>().ok()?;
    Some((num_start, num_end, val))
}

fn optimize_lua_loops(source: &str) -> ScriptLoopOptResult {
    let mut output = String::with_capacity(source.len());
    let mut discarded_loops = 0usize;
    let mut neutralized_loops = 0usize;

    let bytes = source.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            let start = i;
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
            output.push_str(&source[start..i]);
            continue;
        }

        if bytes[i] == b'\'' || bytes[i] == b'"' {
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
            output.push_str(&source[start..i]);
            continue;
        }

        if is_lua_word(bytes, i, "while") {
            if let Some((do_start, do_end)) = find_lua_do_keyword(source, i + 5) {
                if let Some((end_start, end_after)) = find_lua_loop_end(source, do_end, false) {
                    let cond = source[i + 5..do_start].trim();
                    let body = &source[do_end..end_start];
                    let unbounded = matches!(cond, "true" | "1") && !body.contains("break");
                    if unbounded {
                        if !contains_necessary_engine_logic(body) {
                            output.push_str("-- [HotCompile] Bucle pesado Lua sin lógica necesaria descartado");
                            discarded_loops += 1;
                            i = end_after;
                            continue;
                        } else {
                            output.push_str("-- [HotCompile] Bucle infinito Lua neutralizado\nfor _bf_once = 1, 1 do");
                            let inner_opt = optimize_lua_loops(body);
                            discarded_loops += inner_opt.discarded_loops;
                            neutralized_loops += inner_opt.neutralized_loops + 1;
                            output.push_str(&inner_opt.optimized_source);
                            output.push_str("end");
                            i = end_after;
                            continue;
                        }
                    }
                }
            }
        } else if is_lua_word(bytes, i, "for") {
            if let Some((do_start, do_end)) = find_lua_do_keyword(source, i + 3) {
                if let Some((end_start, end_after)) = find_lua_loop_end(source, do_end, false) {
                    let header = &source[i + 3..do_start];
                    let body = &source[do_end..end_start];
                    let in_update = is_lua_inside_on_update(source, i);
                    let safe_cap = if in_update { 64 } else { 256 };

                    if let Some((ls, le, limit)) = parse_lua_numeric_for_limit(header) {
                        if limit > safe_cap {
                            if !contains_necessary_engine_logic(body) {
                                output.push_str("-- [HotCompile] Bucle for pesado Lua descartado");
                                discarded_loops += 1;
                                i = end_after;
                                continue;
                            } else {
                                output.push_str("for");
                                output.push_str(&header[..ls]);
                                output.push_str(&safe_cap.to_string());
                                output.push_str(&header[le..]);
                                output.push_str("do");
                                let inner_opt = optimize_lua_loops(body);
                                discarded_loops += inner_opt.discarded_loops;
                                neutralized_loops += inner_opt.neutralized_loops + 1;
                                output.push_str(&inner_opt.optimized_source);
                                output.push_str("end");
                                i = end_after;
                                continue;
                            }
                        }
                    }
                }
            }
        } else if is_lua_word(bytes, i, "repeat") {
            if let Some((until_start, line_end)) = find_lua_loop_end(source, i + 6, true) {
                let body = &source[i + 6..until_start];
                let until_cond = source[until_start + 5..line_end].trim();
                if matches!(until_cond, "false" | "nil") && !body.contains("break") {
                    if !contains_necessary_engine_logic(body) {
                        output.push_str("-- [HotCompile] Bucle repeat pesado Lua descartado");
                        discarded_loops += 1;
                        i = line_end;
                        continue;
                    } else {
                        output.push_str("-- [HotCompile] Bucle repeat infinito Lua neutralizado\nfor _bf_once = 1, 1 do");
                        let inner_opt = optimize_lua_loops(body);
                        discarded_loops += inner_opt.discarded_loops;
                        neutralized_loops += inner_opt.neutralized_loops + 1;
                        output.push_str(&inner_opt.optimized_source);
                        output.push_str("end");
                        i = line_end;
                        continue;
                    }
                }
            }
        }

        let ch = source[i..].chars().next().unwrap();
        output.push(ch);
        i += ch.len_utf8();
    }

    ScriptLoopOptResult {
        optimized_source: output,
        discarded_loops,
        neutralized_loops,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discards_useless_heavy_js_loop_and_keeps_small_loop() {
        let ts = r#"
function onUpdate(dt: number) {
    let acc = 0;
    for (let i = 0; i < 1000000; i++) {
        acc += i;
    }
    for (let lane = 0; lane < 4; lane++) {
        setEntityProp("player_main", "opacity", 1.0);
    }
}
"#;
        let res = optimize_single_script(ScriptLanguage::TypeScript, ts);
        assert_eq!(res.discarded_loops, 1);
        assert!(!res.optimized_source.contains("1000000"));
        assert!(res.optimized_source.contains("lane < 4"));
        assert!(res.optimized_source.contains("setEntityProp"));
    }

    #[test]
    fn neutralizes_infinite_loop_with_necessary_engine_logic() {
        let js = r#"
function onUpdate(dt) {
    while (true) {
        setCameraZoom(1.08);
    }
}
"#;
        let res = optimize_single_script(ScriptLanguage::JavaScript, js);
        assert_eq!(res.neutralized_loops, 1);
        assert!(!res.optimized_source.contains("while (true)"));
        assert!(res.optimized_source.contains("setCameraZoom(1.08)"));
    }

    #[test]
    fn discards_useless_lua_loop_and_caps_heavy_necessary_lua_loop() {
        let lua = r#"
function onUpdate(dt)
    local x = 0
    while true do
        x = x + 1
    end
    for i = 1, 50000 do
        setEntityProp("stage_bg", "opacity", 0.9)
    end
end
"#;
        let res = optimize_single_script(ScriptLanguage::Lua, lua);
        assert_eq!(res.discarded_loops, 1);
        assert_eq!(res.neutralized_loops, 1);
        assert!(!res.optimized_source.contains("while true do"));
        assert!(res.optimized_source.contains("1, 64"));
        assert!(res.optimized_source.contains("setEntityProp"));
    }
}
