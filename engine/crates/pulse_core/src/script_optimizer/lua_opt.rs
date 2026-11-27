//! Archivo: engine/crates/pulse_core/src/script_optimizer/lua_opt.rs
//! Propósito: Analizador y optimizador estático de bucles `while`, `for` y `repeat..until`
//! en scripts Lua 5.4, podando ciclos vacíos y acotando bucles costosos sin alterar la sintaxis.

use super::ScriptLoopOptResult;
use super::scanner::contains_necessary_engine_logic;

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

/// Optimiza bucles pesados o infinitos en código fuente Lua 5.4.
pub(super) fn optimize_lua_loops(source: &str) -> ScriptLoopOptResult {
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
