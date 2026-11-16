//! Archivo: engine/crates/pulse_scripting/src/ts_transpiler.rs
//! Propósito: Transpila scripts comunitarios escritos en TypeScript (`.ts`) directamente a
//! JavaScript puro dentro de Rust usando `oxc_parser` + `oxc_transformer` + `oxc_codegen`
//! (licencia MIT), sin requerir Node.js instalado en el teléfono Android del usuario.

use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};
use std::path::Path;

/// Convierte código TypeScript a ES2022 JavaScript ejecutable por QuickJS-NG.
pub fn transpile_typescript_to_js(filename: &str, ts_source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parsed = Parser::new(&allocator, ts_source, source_type).parse();

    if parsed.diagnostics.has_errors() {
        let msgs: Vec<String> = parsed.diagnostics.errors().map(|e| e.to_string()).collect();
        return Err(format!("Error de sintaxis TypeScript: {}", msgs.join("; ")));
    }

    let mut program = parsed.program;
    let semantic = SemanticBuilder::new().build(&program);
    if semantic.diagnostics.has_errors() {
        let msgs: Vec<String> = semantic.diagnostics.errors().map(|e| e.to_string()).collect();
        return Err(format!("Error semántico TypeScript: {}", msgs.join("; ")));
    }

    let scoping = semantic.semantic.into_scoping();
    let transform_options = TransformOptions::default();
    let ret = Transformer::new(&allocator, Path::new(filename), &transform_options)
        .build_with_scoping(scoping, &mut program);

    if ret.diagnostics.has_errors() {
        let msgs: Vec<String> = ret.diagnostics.errors().map(|e| e.to_string()).collect();
        return Err(format!("Error transpilando TypeScript: {}", msgs.join("; ")));
    }

    let js_output = Codegen::new().build(&program).code;
    Ok(js_output)
}
