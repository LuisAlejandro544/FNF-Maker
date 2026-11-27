//! Archivo: engine/crates/pulse_core/src/script_optimizer/js_opt.rs
//! Propósito: Analizador y optimizador estático de bucles `for` y `while` en scripts
//! JavaScript y TypeScript, neutralizando iteraciones infinitas o costosas dentro y fuera de `onUpdate`.

use super::ScriptLoopOptResult;
use super::scanner::{
    contains_necessary_engine_logic, find_matching_brace, find_matching_paren, is_word_boundary,
    skip_ws_and_js_comments,
};

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

/// Optimiza bucles pesados o infinitos en código fuente JavaScript / TypeScript.
pub(super) fn optimize_js_ts_loops(source: &str) -> ScriptLoopOptResult {
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
