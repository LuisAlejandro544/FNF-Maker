//! Archivo: engine/crates/pulse_core/src/mod_descriptors.rs
//! Propósito: Interpreta archivos descriptores `.json` y `.xml` propios de carpetas de mods
//! (`characters/*.json`, `characters/*.xml`, `stages/*.json`, `stages/*.xml`) que instruyen
//! la posición exacta de cada sprite en el escenario, su escala, volteo, enfoque de cámara,
//! orden Z y los `offsets [x, y]` individuales por animación (`Idle`, `Left`, `Down`, `Up`, `Right`, `Miss`).

use crate::sparrow::CharacterAnimKind;
use crate::stage::EntityRole;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Definición individual de una animación dentro de un descriptor de personaje.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterAnimDescriptor {
    pub anim_key: Option<CharacterAnimKind>,
    pub anim_name_raw: String,
    pub prefix: String,
    pub fps: f32,
    pub looped: bool,
    /// Desplazamiento normalizado al espacio BeatForge (`+x` derecha, `+y` abajo).
    pub offset: [f32; 2],
    pub flip_x: Option<bool>,
}

/// Descriptor completo de personaje (`.json` Psych/V-Slice o `.xml` Codename `<character>`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterDescriptor {
    pub format_label: String,
    /// Posición indicada en el archivo (`[x, y]`).
    pub position: Option<[f32; 2]>,
    /// Indica si `position` es un desplazamiento relativo al slot del escenario (true en Psych/Codename).
    pub is_relative_position: bool,
    pub camera_position: Option<[f32; 2]>,
    pub scale: Option<f32>,
    pub flip_x: Option<bool>,
    pub animations: Vec<CharacterAnimDescriptor>,
}

/// Prop o capa decorativa/fondo definida dentro de un archivo de escenario (`.json` / `.xml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StagePropDescriptor {
    pub name: String,
    pub asset_hint: String,
    pub position: [f32; 2],
    pub scale: f32,
    pub scroll_factor: f32,
    pub z_index: i32,
    pub flip_x: bool,
    pub alpha: f32,
}

/// Descriptor completo de escenario (`stages/*.json` o `stages/*.xml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageDescriptor {
    pub format_label: String,
    pub default_zoom: Option<f32>,
    pub camera_start: Option<[f32; 2]>,
    pub player_pos: Option<[f32; 2]>,
    pub opponent_pos: Option<[f32; 2]>,
    pub gf_pos: Option<[f32; 2]>,
    pub is_bottom_center_coords: bool,
    pub hide_gf: bool,
    pub player_cam_offset: Option<[f32; 2]>,
    pub opponent_cam_offset: Option<[f32; 2]>,
    pub gf_cam_offset: Option<[f32; 2]>,
    pub player_z: Option<i32>,
    pub opponent_z: Option<i32>,
    pub gf_z: Option<i32>,
    pub props: Vec<StagePropDescriptor>,
}

/// Resultado de autodetección para cualquier descriptor `.json` o `.xml` de mod.
#[derive(Debug, Clone, PartialEq)]
pub enum DetectedModDescriptor {
    Character(CharacterDescriptor),
    Stage(StageDescriptor),
}

/// Mapea nombres estándar de animaciones en mods (`idle`, `singLEFT`, `singUPmiss`, etc.)
/// al enum `CharacterAnimKind` de BeatForge Studio.
pub fn map_anim_name_to_kind(raw: &str) -> Option<CharacterAnimKind> {
    let clean = raw.trim().to_ascii_lowercase().replace([' ', '-', '_'], "");
    if clean.is_empty() {
        return None;
    }

    let is_miss = clean.contains("miss") || clean.contains("fallo");
    if is_miss {
        if clean.contains("left") || clean.contains("izq") {
            return Some(CharacterAnimKind::MissLeft);
        }
        if clean.contains("down") || clean.contains("abajo") {
            return Some(CharacterAnimKind::MissDown);
        }
        if clean.contains("up") || clean.contains("arriba") {
            return Some(CharacterAnimKind::MissUp);
        }
        if clean.contains("right") || clean.contains("der") {
            return Some(CharacterAnimKind::MissRight);
        }
        return None;
    }

    if clean == "idle"
        || clean == "dance"
        || clean == "danceleft"
        || clean == "danceright"
        || clean == "reposo"
        || clean.starts_with("idlealt")
    {
        return Some(CharacterAnimKind::Idle);
    }

    if clean == "singleft" || clean == "left" || clean == "izquierda" || clean.starts_with("singleft") {
        return Some(CharacterAnimKind::Left);
    }
    if clean == "singdown" || clean == "down" || clean == "abajo" || clean.starts_with("singdown") {
        return Some(CharacterAnimKind::Down);
    }
    if clean == "singup" || clean == "up" || clean == "arriba" || clean.starts_with("singup") {
        return Some(CharacterAnimKind::Up);
    }
    if clean == "singright" || clean == "right" || clean == "derecha" || clean.starts_with("singright") {
        return Some(CharacterAnimKind::Right);
    }

    None
}

/// Devuelve la posición base predeterminada de BeatForge (`1280x720`) para cada rol.
pub fn default_role_anchor(role: EntityRole) -> [f32; 2] {
    match role {
        EntityRole::Background => [640.0, 360.0],
        EntityRole::Decoration => [640.0, 430.0],
        EntityRole::Opponent => [300.0, 420.0],
        EntityRole::Player => [980.0, 430.0],
    }
}

/// Convierte coordenadas de un `stage.json` / `stage.xml` hacia el lienzo virtual `1280x720`.
pub fn normalize_stage_world_pos(
    raw_pos: [f32; 2],
    role: EntityRole,
    is_bottom_center_coords: bool,
) -> [f32; 2] {
    let [rx, ry] = raw_pos;
    let (cx, cy) = if is_bottom_center_coords {
        // V-Slice: origen en pies del personaje dentro de mundo con cámara alta (~1400x1000)
        let x = rx.clamp(-400.0, 1700.0);
        let y = (ry * 0.56).clamp(120.0, 680.0);
        (x, y)
    } else {
        // Psych / Codename: origen suele ser esquina superior izquierda de sprite (~280x360)
        let half_w = match role {
            EntityRole::Background => 640.0,
            EntityRole::Decoration => 120.0,
            EntityRole::Player | EntityRole::Opponent => 140.0,
        };
        let half_h = match role {
            EntityRole::Background => 360.0,
            EntityRole::Decoration => 110.0,
            EntityRole::Player | EntityRole::Opponent => 180.0,
        };
        let x = (rx + half_w).clamp(-600.0, 2000.0);
        let y = (ry + half_h).clamp(-400.0, 1200.0);
        (x, y)
    };
    [cx, cy]
}

/// Calcula la posición final de una entidad al aplicar `position` de un descriptor de personaje.
pub fn apply_character_position(
    current_pos: [f32; 2],
    char_pos: [f32; 2],
    is_relative: bool,
    role: EntityRole,
) -> [f32; 2] {
    let [px, py] = char_pos;
    if !px.is_finite() || !py.is_finite() {
        return current_pos;
    }

    // Si las coordenadas son grandes y positivas en X (ej. `[950, 440]` puestas explícitamente para 1280x720),
    // respetarlas directamente como posición absoluta en el lienzo.
    if !is_relative || (px.abs() > 460.0 && py > 40.0) {
        return [px.clamp(-1000.0, 2600.0), py.clamp(-1000.0, 1800.0)];
    }

    let anchor = default_role_anchor(role);
    let nx = (anchor[0] + px * 0.55).clamp(-1000.0, 2600.0);
    let ny = (anchor[1] + py * 0.25).clamp(-1000.0, 1800.0);
    [nx, ny]
}

/// Detecta e interpreta un archivo `.json` o `.xml` de configuración de personaje o escenario.
pub fn parse_mod_descriptor_str(raw: &str) -> Result<DetectedModDescriptor, String> {
    let trimmed = raw.trim_start_matches('\u{FEFF}').trim();
    if trimmed.is_empty() {
        return Err("El archivo de configuración está vacío".to_string());
    }

    if trimmed.starts_with('{') {
        let root: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("JSON de configuración inválido: {}", e))?;
        let obj = root
            .as_object()
            .ok_or_else(|| "El JSON debe tener un objeto raíz".to_string())?;

        if looks_like_stage_json(obj) {
            return parse_stage_json_obj(obj).map(DetectedModDescriptor::Stage);
        }
        if looks_like_character_json(obj) {
            return parse_character_json_obj(obj).map(DetectedModDescriptor::Character);
        }
        return Err("El JSON no coincide con un descriptor de personaje (animations/position) ni de escenario (boyfriend/opponent/characters/props)".to_string());
    }

    if trimmed.starts_with('<') {
        let doc = roxmltree::Document::parse(trimmed)
            .map_err(|e| format!("XML de configuración inválido: {}", e))?;
        let root = doc.root_element();
        let tag = root.tag_name().name();
        if tag.eq_ignore_ascii_case("character") {
            return parse_character_xml_root(root).map(DetectedModDescriptor::Character);
        }
        if tag.eq_ignore_ascii_case("stage") {
            return parse_stage_xml_root(root).map(DetectedModDescriptor::Stage);
        }
        return Err(format!(
            "Etiqueta raíz XML '<{}>' no es <character> ni <stage>",
            tag
        ));
    }

    Err("Formato no reconocido (se esperaba JSON o XML de personaje/escenario)".to_string())
}

/// Interpreta específicamente un descriptor de personaje (`.json` o `<character>` `.xml`).
pub fn parse_character_descriptor_str(raw: &str) -> Result<CharacterDescriptor, String> {
    match parse_mod_descriptor_str(raw)? {
        DetectedModDescriptor::Character(c) => Ok(c),
        DetectedModDescriptor::Stage(_) => {
            Err("El archivo corresponde a un escenario (<stage>/stage.json), impórtalo desde la pestaña Escena".to_string())
        }
    }
}

/// Interpreta específicamente un descriptor de escenario (`.json` o `<stage>` `.xml`).
pub fn parse_stage_descriptor_str(raw: &str) -> Result<StageDescriptor, String> {
    match parse_mod_descriptor_str(raw)? {
        DetectedModDescriptor::Stage(s) => Ok(s),
        DetectedModDescriptor::Character(_) => {
            Err("El archivo corresponde a un personaje (<character>/character.json), impórtalo desde Sprites".to_string())
        }
    }
}

fn looks_like_character_json(obj: &serde_json::Map<String, Value>) -> bool {
    obj.get("animations").is_some_and(|v| v.is_array())
        || obj.contains_key("camera_position")
        || obj.contains_key("healthicon")
        || obj.contains_key("sing_duration")
        || (obj.contains_key("position") && obj.contains_key("image"))
        || obj.contains_key("startingAnimation")
}

fn looks_like_stage_json(obj: &serde_json::Map<String, Value>) -> bool {
    obj.contains_key("boyfriend")
        || obj.contains_key("opponent")
        || obj.contains_key("girlfriend")
        || obj.contains_key("characters")
        || obj.get("props").is_some_and(|v| v.is_array())
        || obj.contains_key("defaultZoom")
        || obj.contains_key("cameraZoom")
}

fn parse_character_json_obj(obj: &serde_json::Map<String, Value>) -> Result<CharacterDescriptor, String> {
    let is_vslice = obj.contains_key("assetPath")
        || obj.contains_key("startingAnimation")
        || obj
            .get("animations")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .is_some_and(|item| item.get("prefix").is_some() || item.get("looped").is_some());

    let position = parse_vec2(obj.get("position")).or_else(|| parse_vec2(obj.get("offsets")));
    let camera_position = parse_vec2(obj.get("camera_position"));
    let scale = parse_f32_val(obj.get("scale")).map(|s| s.clamp(0.05, 6.0));
    let flip_x = obj
        .get("flip_x")
        .or_else(|| obj.get("flipX"))
        .and_then(|v| v.as_bool());

    let mut animations = Vec::new();
    if let Some(arr) = obj.get("animations").and_then(|v| v.as_array()) {
        for item in arr {
            let Some(aobj) = item.as_object() else {
                continue;
            };
            let anim_raw = aobj
                .get("anim")
                .or_else(|| aobj.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let prefix = aobj
                .get("prefix")
                .or_else(|| {
                    if aobj.contains_key("anim") {
                        aobj.get("name")
                    } else {
                        None
                    }
                })
                .and_then(|v| v.as_str())
                .unwrap_or(&anim_raw)
                .trim()
                .to_string();

            if anim_raw.is_empty() && prefix.is_empty() {
                continue;
            }

            let fps = parse_f32_val(aobj.get("fps")).unwrap_or(24.0).clamp(1.0, 120.0);
            let looped = aobj
                .get("loop")
                .or_else(|| aobj.get("looped"))
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| anim_raw.to_ascii_lowercase().contains("idle"));
            let raw_off = parse_vec2(aobj.get("offsets")).unwrap_or([0.0, 0.0]);
            // Psych resta offsets (`-x, -y`), mientras V-Slice suma offsets (`+x, +y`).
            let offset = if is_vslice {
                raw_off
            } else {
                [-raw_off[0], -raw_off[1]]
            };
            let anim_flip = aobj.get("flipX").and_then(|v| v.as_bool());

            animations.push(CharacterAnimDescriptor {
                anim_key: map_anim_name_to_kind(&anim_raw),
                anim_name_raw: anim_raw,
                prefix,
                fps,
                looped,
                offset,
                flip_x: anim_flip,
            });
        }
    }

    if animations.is_empty() && position.is_none() && scale.is_none() && flip_x.is_none() {
        return Err("El JSON de personaje no contiene animaciones ni posición".to_string());
    }

    Ok(CharacterDescriptor {
        format_label: if is_vslice {
            "Personaje JSON (V-Slice)".to_string()
        } else {
            "Personaje JSON (Psych)".to_string()
        },
        position,
        is_relative_position: !is_vslice,
        camera_position,
        scale,
        flip_x,
        animations,
    })
}

fn parse_character_xml_root(root: roxmltree::Node) -> Result<CharacterDescriptor, String> {
    let x = parse_f32_opt(root.attribute("x"));
    let y = parse_f32_opt(root.attribute("y"));
    let position = match (x, y) {
        (Some(vx), Some(vy)) => Some([vx, vy]),
        (Some(vx), None) => Some([vx, 0.0]),
        (None, Some(vy)) => Some([0.0, vy]),
        (None, None) => None,
    };

    let cam_x = parse_f32_opt(root.attribute("camx"));
    let cam_y = parse_f32_opt(root.attribute("camy"));
    let camera_position = match (cam_x, cam_y) {
        (Some(cx), Some(cy)) => Some([cx, cy]),
        (Some(cx), None) => Some([cx, 0.0]),
        (None, Some(cy)) => Some([0.0, cy]),
        (None, None) => None,
    };

    let scale = parse_f32_opt(root.attribute("scale")).map(|s| s.clamp(0.05, 6.0));
    let flip_x = parse_bool_opt(root.attribute("flipX").or_else(|| root.attribute("flip_x")));

    let mut animations = Vec::new();
    for node in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("anim"))
    {
        let anim_raw = node.attribute("name").unwrap_or("").trim().to_string();
        let prefix = node
            .attribute("anim")
            .or_else(|| node.attribute("prefix"))
            .unwrap_or(&anim_raw)
            .trim()
            .to_string();
        if anim_raw.is_empty() && prefix.is_empty() {
            continue;
        }
        let fps = parse_f32_opt(node.attribute("fps"))
            .unwrap_or(24.0)
            .clamp(1.0, 120.0);
        let looped = parse_bool_opt(node.attribute("loop"))
            .unwrap_or_else(|| anim_raw.to_ascii_lowercase().contains("idle"));
        let ox = parse_f32_opt(node.attribute("x")).unwrap_or(0.0);
        let oy = parse_f32_opt(node.attribute("y")).unwrap_or(0.0);
        let anim_flip = parse_bool_opt(node.attribute("flipX"));

        animations.push(CharacterAnimDescriptor {
            anim_key: map_anim_name_to_kind(&anim_raw),
            anim_name_raw: anim_raw,
            prefix,
            fps,
            looped,
            offset: [-ox, -oy],
            flip_x: anim_flip,
        });
    }

    if animations.is_empty() && position.is_none() && scale.is_none() && flip_x.is_none() {
        return Err("El XML <character> no contiene animaciones ni atributos de posición".to_string());
    }

    Ok(CharacterDescriptor {
        format_label: "Personaje XML (Codename)".to_string(),
        position,
        is_relative_position: true,
        camera_position,
        scale,
        flip_x,
        animations,
    })
}

fn parse_stage_json_obj(obj: &serde_json::Map<String, Value>) -> Result<StageDescriptor, String> {
    let has_characters_obj = obj.get("characters").and_then(|v| v.as_object()).is_some();
    let default_zoom = parse_f32_val(obj.get("defaultZoom").or_else(|| obj.get("cameraZoom")))
        .map(|z| z.clamp(0.35, 2.5));
    let hide_gf = obj
        .get("hide_girlfriend")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut player_pos = parse_vec2(obj.get("boyfriend"));
    let mut opponent_pos = parse_vec2(obj.get("opponent"));
    let mut gf_pos = parse_vec2(obj.get("girlfriend"));

    let mut player_cam_offset = parse_vec2(obj.get("camera_boyfriend"));
    let mut opponent_cam_offset = parse_vec2(obj.get("camera_opponent"));
    let mut gf_cam_offset = parse_vec2(obj.get("camera_girlfriend"));

    let mut player_z = None;
    let mut opponent_z = None;
    let mut gf_z = None;

    if let Some(chars) = obj.get("characters").and_then(|v| v.as_object()) {
        if let Some(bf) = chars.get("bf").or_else(|| chars.get("boyfriend")).and_then(|v| v.as_object()) {
            player_pos = parse_vec2(bf.get("position")).or(player_pos);
            player_cam_offset = parse_vec2(bf.get("cameraOffsets")).or(player_cam_offset);
            player_z = parse_i32_val(bf.get("zIndex"));
        }
        if let Some(dad) = chars.get("dad").or_else(|| chars.get("opponent")).and_then(|v| v.as_object()) {
            opponent_pos = parse_vec2(dad.get("position")).or(opponent_pos);
            opponent_cam_offset = parse_vec2(dad.get("cameraOffsets")).or(opponent_cam_offset);
            opponent_z = parse_i32_val(dad.get("zIndex"));
        }
        if let Some(gf) = chars.get("gf").or_else(|| chars.get("girlfriend")).and_then(|v| v.as_object()) {
            gf_pos = parse_vec2(gf.get("position")).or(gf_pos);
            gf_cam_offset = parse_vec2(gf.get("cameraOffsets")).or(gf_cam_offset);
            gf_z = parse_i32_val(gf.get("zIndex"));
        }
    }

    let mut props = Vec::new();
    if let Some(arr) = obj.get("props").and_then(|v| v.as_array()) {
        for (idx, item) in arr.iter().enumerate() {
            let Some(pobj) = item.as_object() else {
                continue;
            };
            let name = pobj
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("prop")
                .to_string();
            let asset_hint = pobj
                .get("assetPath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let pos = parse_vec2(pobj.get("position")).unwrap_or([640.0, 360.0]);
            let scale = parse_vec2(pobj.get("scale"))
                .map(|[sx, sy]| ((sx + sy) * 0.5).clamp(0.05, 6.0))
                .or_else(|| parse_f32_val(pobj.get("scale")).map(|s| s.clamp(0.05, 6.0)))
                .unwrap_or(1.0);
            let scroll_factor = parse_vec2(pobj.get("scroll"))
                .map(|[sx, sy]| ((sx + sy) * 0.5).clamp(0.0, 2.0))
                .unwrap_or(1.0);
            let z_index = parse_i32_val(pobj.get("zIndex")).unwrap_or(idx as i32 * 5);
            let flip_x = pobj.get("flipX").and_then(|v| v.as_bool()).unwrap_or(false);
            let alpha = parse_f32_val(pobj.get("alpha"))
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);

            props.push(StagePropDescriptor {
                name,
                asset_hint,
                position: pos,
                scale,
                scroll_factor,
                z_index,
                flip_x,
                alpha,
            });
        }
    }

    Ok(StageDescriptor {
        format_label: if has_characters_obj {
            "Escenario JSON (V-Slice)".to_string()
        } else {
            "Escenario JSON (Psych)".to_string()
        },
        default_zoom,
        camera_start: None,
        player_pos,
        opponent_pos,
        gf_pos,
        is_bottom_center_coords: has_characters_obj,
        hide_gf,
        player_cam_offset,
        opponent_cam_offset,
        gf_cam_offset,
        player_z,
        opponent_z,
        gf_z,
        props,
    })
}

fn parse_stage_xml_root(root: roxmltree::Node) -> Result<StageDescriptor, String> {
    let default_zoom = parse_f32_opt(root.attribute("zoom")).map(|z| z.clamp(0.35, 2.5));
    let camera_start = match (
        parse_f32_opt(root.attribute("startCamPosX")),
        parse_f32_opt(root.attribute("startCamPosY")),
    ) {
        (Some(cx), Some(cy)) => Some([cx, cy]),
        _ => None,
    };

    let mut player_pos = None;
    let mut opponent_pos = None;
    let mut gf_pos = None;
    let mut player_cam_offset = None;
    let mut opponent_cam_offset = None;
    let mut gf_cam_offset = None;
    let mut props = Vec::new();

    for (idx, node) in root.children().filter(|n| n.is_element()).enumerate() {
        let tag = node.tag_name().name().to_ascii_lowercase();
        let pos = match (parse_f32_opt(node.attribute("x")), parse_f32_opt(node.attribute("y"))) {
            (Some(x), Some(y)) => Some([x, y]),
            (Some(x), None) => Some([x, 0.0]),
            (None, Some(y)) => Some([0.0, y]),
            (None, None) => None,
        };
        let cam_off = match (
            parse_f32_opt(node.attribute("camxoffset")),
            parse_f32_opt(node.attribute("camyoffset")),
        ) {
            (Some(cx), Some(cy)) => Some([cx, cy]),
            _ => None,
        };

        match tag.as_str() {
            "boyfriend" | "bf" | "player" => {
                player_pos = pos.or(player_pos);
                player_cam_offset = cam_off.or(player_cam_offset);
            }
            "dad" | "opponent" => {
                opponent_pos = pos.or(opponent_pos);
                opponent_cam_offset = cam_off.or(opponent_cam_offset);
            }
            "girlfriend" | "gf" => {
                gf_pos = pos.or(gf_pos);
                gf_cam_offset = cam_off.or(gf_cam_offset);
            }
            "sprite" | "solid" | "box" => {
                let name = node
                    .attribute("name")
                    .unwrap_or("prop")
                    .trim()
                    .to_string();
                let asset_hint = node.attribute("sprite").unwrap_or("").trim().to_string();
                let scale = parse_f32_opt(node.attribute("scale"))
                    .unwrap_or(1.0)
                    .clamp(0.05, 6.0);
                let sx = parse_f32_opt(node.attribute("scrollx")).unwrap_or(1.0);
                let sy = parse_f32_opt(node.attribute("scrolly")).unwrap_or(1.0);
                let flip_x = parse_bool_opt(node.attribute("flipX")).unwrap_or(false);
                let alpha = parse_f32_opt(node.attribute("alpha"))
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0);

                props.push(StagePropDescriptor {
                    name,
                    asset_hint,
                    position: pos.unwrap_or([640.0, 360.0]),
                    scale,
                    scroll_factor: ((sx + sy) * 0.5).clamp(0.0, 2.0),
                    z_index: idx as i32 * 5,
                    flip_x,
                    alpha,
                });
            }
            _ => {}
        }
    }

    Ok(StageDescriptor {
        format_label: "Escenario XML (Codename)".to_string(),
        default_zoom,
        camera_start,
        player_pos,
        opponent_pos,
        gf_pos,
        is_bottom_center_coords: false,
        hide_gf: false,
        player_cam_offset,
        opponent_cam_offset,
        gf_cam_offset,
        player_z: None,
        opponent_z: None,
        gf_z: None,
        props,
    })
}

fn parse_vec2(val: Option<&Value>) -> Option<[f32; 2]> {
    let arr = val?.as_array()?;
    if arr.len() < 2 {
        return None;
    }
    let x = parse_f32_val(arr.first())?;
    let y = parse_f32_val(arr.get(1))?;
    if x.is_finite() && y.is_finite() {
        Some([x, y])
    } else {
        None
    }
}

fn parse_f32_val(val: Option<&Value>) -> Option<f32> {
    let v = val?;
    if let Some(n) = v.as_f64() {
        return Some(n as f32);
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<f32>().ok();
    }
    None
}

fn parse_i32_val(val: Option<&Value>) -> Option<i32> {
    let v = val?;
    if let Some(n) = v.as_i64() {
        return Some(n as i32);
    }
    if let Some(n) = v.as_f64() {
        return Some(n.round() as i32);
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<i32>().ok();
    }
    None
}

fn parse_f32_opt(raw: Option<&str>) -> Option<f32> {
    raw.and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
}

fn parse_bool_opt(raw: Option<&str>) -> Option<bool> {
    let s = raw?.trim().to_ascii_lowercase();
    match s.as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_psych_character_json_with_position_and_anim_offsets() {
        let json = r#"{
            "animations": [
                { "anim": "idle", "name": "Hero Idle", "fps": 24, "loop": false, "offsets": [0, 0] },
                { "anim": "singLEFT", "name": "Hero Left", "fps": 24, "loop": false, "offsets": [12, -8] },
                { "anim": "singUPmiss", "name": "Hero MissUp", "fps": 24, "loop": false, "offsets": [-20, 30] }
            ],
            "flip_x": true,
            "scale": 1.15,
            "position": [40, 200],
            "camera_position": [15, -40]
        }"#;

        let desc = parse_character_descriptor_str(json).expect("valid psych char");
        assert!(desc.is_relative_position);
        assert_eq!(desc.position, Some([40.0, 200.0]));
        assert_eq!(desc.camera_position, Some([15.0, -40.0]));
        assert_eq!(desc.scale, Some(1.15));
        assert_eq!(desc.flip_x, Some(true));
        assert_eq!(desc.animations.len(), 3);
        assert_eq!(desc.animations[1].anim_key, Some(CharacterAnimKind::Left));
        assert_eq!(desc.animations[1].prefix, "Hero Left");
        assert_eq!(desc.animations[1].offset, [-12.0, 8.0]);
        assert_eq!(desc.animations[2].anim_key, Some(CharacterAnimKind::MissUp));
        assert_eq!(desc.animations[2].offset, [20.0, -30.0]);
    }

    #[test]
    fn parses_codename_character_xml_with_xy_and_anims() {
        let xml = r#"
        <character x="20" y="100" camx="10" camy="-25" flipX="false" scale="0.95">
            <anim name="idle" anim="Rival idle" fps="24" loop="true" x="0" y="0"/>
            <anim name="singRIGHT" anim="Rival right" fps="24" loop="false" x="-14" y="22"/>
        </character>
        "#;

        let desc = parse_character_descriptor_str(xml).expect("valid codename xml");
        assert_eq!(desc.position, Some([20.0, 100.0]));
        assert_eq!(desc.camera_position, Some([10.0, -25.0]));
        assert_eq!(desc.scale, Some(0.95));
        assert_eq!(desc.flip_x, Some(false));
        assert_eq!(desc.animations[0].anim_key, Some(CharacterAnimKind::Idle));
        assert!(desc.animations[0].looped);
        assert_eq!(desc.animations[1].anim_key, Some(CharacterAnimKind::Right));
        assert_eq!(desc.animations[1].offset, [14.0, -22.0]);
    }

    #[test]
    fn parses_psych_and_vslice_stage_json_descriptors() {
        let psych_stage = r#"{
            "defaultZoom": 0.9,
            "boyfriend": [820, 220],
            "girlfriend": [450, 140],
            "opponent": [110, 220],
            "hide_girlfriend": true
        }"#;
        let st1 = parse_stage_descriptor_str(psych_stage).expect("psych stage");
        assert_eq!(st1.default_zoom, Some(0.9));
        assert_eq!(st1.player_pos, Some([820.0, 220.0]));
        assert_eq!(st1.opponent_pos, Some([110.0, 220.0]));
        assert!(st1.hide_gf);
        assert!(!st1.is_bottom_center_coords);

        let vslice_stage = r#"{
            "cameraZoom": 1.1,
            "props": [
                { "name": "stageBack", "assetPath": "stages/main/back", "position": [0, 0], "scale": [1.0, 1.0], "scroll": [0.9, 0.9], "zIndex": 10 }
            ],
            "characters": {
                "bf": { "zIndex": 300, "position": [990, 880], "cameraOffsets": [-100, -100] },
                "dad": { "zIndex": 200, "position": [335, 880], "cameraOffsets": [150, -100] }
            }
        }"#;
        let st2 = parse_stage_descriptor_str(vslice_stage).expect("vslice stage");
        assert!(st2.is_bottom_center_coords);
        assert_eq!(st2.default_zoom, Some(1.1));
        assert_eq!(st2.player_z, Some(300));
        assert_eq!(st2.props.len(), 1);
    }
}
