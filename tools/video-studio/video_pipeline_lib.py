"""
Archivo: tools/video-studio/video_pipeline_lib.py
Propósito: Funciones puras y deterministas para el pipeline de edición de video
           (compatible con archivos .mp4 largos y pesados):
           - Cálculo de segmentos activos invirtiendo silencios con padding seguro
           - Generación y validación de archivos edit.json (edición declarativa)
           - Generación de subtítulos estilizados .ass (karaoke / frase vertical)
           - Construcción segura de grafos FFmpeg (trim/concat, 9:16 crop, loudnorm)
"""

from __future__ import annotations

from dataclasses import dataclass, asdict
from typing import Iterable, List, Dict, Any, Optional, Tuple


@dataclass(frozen=True)
class TimeSegment:
    """Segmento temporal validado [start, end] en segundos."""

    start: float
    end: float
    label: str = ""

    @property
    def duration(self) -> float:
        return max(0.0, self.end - self.start)


def invert_silences_to_keep_segments(
    total_duration: float,
    silences: Iterable[Tuple[float, float]],
    min_silence_duration: float = 0.65,
    pad_before: float = 0.12,
    pad_after: float = 0.14,
    min_keep_duration: float = 0.25,
    merge_gap: float = 0.18,
) -> List[TimeSegment]:
    """
    Convierte intervalos de silencio [(inicio, fin), ...] en intervalos de voz/acción
    a conservar, aplicando margen antes/después para no cortar sílabas ni respiraciones bruscas.
    """
    if total_duration <= 0:
        return []

    filtered_silences: List[Tuple[float, float]] = []
    for raw_start, raw_end in silences:
        s = max(0.0, min(total_duration, float(raw_start)))
        e = max(0.0, min(total_duration, float(raw_end)))
        if e - s < min_silence_duration:
            continue
        # Conserva padding alrededor de la voz -> reduce la porción de silencio cortable
        cut_s = s + max(0.0, pad_after)
        cut_e = e - max(0.0, pad_before)
        if cut_e > cut_s:
            filtered_silences.append((cut_s, cut_e))

    filtered_silences.sort(key=lambda pair: pair[0])

    merged_cuts: List[Tuple[float, float]] = []
    for s, e in filtered_silences:
        if merged_cuts and s <= merged_cuts[-1][1]:
            merged_cuts[-1] = (merged_cuts[-1][0], max(merged_cuts[-1][1], e))
        else:
            merged_cuts.append((s, e))

    keeps: List[TimeSegment] = []
    cursor = 0.0
    for cut_s, cut_e in merged_cuts:
        if cut_s - cursor >= min_keep_duration:
            keeps.append(TimeSegment(start=round(cursor, 3), end=round(cut_s, 3)))
        cursor = max(cursor, cut_e)

    if total_duration - cursor >= min_keep_duration:
        keeps.append(TimeSegment(start=round(cursor, 3), end=round(total_duration, 3)))

    # Fusiona bloques muy cercanos para evitar microcortes molestos
    coalesced: List[TimeSegment] = []
    for seg in keeps:
        if coalesced and (seg.start - coalesced[-1].end) <= merge_gap:
            coalesced[-1] = TimeSegment(start=coalesced[-1].start, end=seg.end, label=coalesced[-1].label)
        else:
            coalesced.append(seg)

    return coalesced


def format_ass_timestamp(seconds: float) -> str:
    """Formatea segundos a marca temporal ASS H:MM:SS.cs (centésimas)."""
    clamped = max(0.0, seconds)
    total_cs = int(round(clamped * 100))
    cs = total_cs % 100
    total_s = total_cs // 100
    s = total_s % 60
    total_m = total_s // 60
    m = total_m % 60
    h = total_m // 60
    return f"{h}:{m:02d}:{s:02d}.{cs:02d}"


def escape_ass_text(text: str) -> str:
    """Escapa caracteres conflictivos para subtítulos SSA/ASS."""
    return text.replace("\\", "\\\\").replace("{", "\\{").replace("}", "\\}").replace("\n", "\\N")


def build_ass_subtitles_from_words(
    segments: List[Dict[str, Any]],
    play_res_x: int = 1080,
    play_res_y: int = 1920,
    max_words_per_cue: int = 4,
    font_name: str = "DejaVu Sans",
    font_size: int = 74,
    margin_v: int = 340,
) -> str:
    """
    Genera un archivo .ass con resaltado palabra por palabra (estilo karaoke amarillo/blanco
    con contorno oscuro) ideal para Reels, Shorts y explicativos en vertical u horizontal.
    """
    header = "\n".join(
        [
            "[Script Info]",
            "ScriptType: v4.00+",
            f"PlayResX: {play_res_x}",
            f"PlayResY: {play_res_y}",
            "ScaledBorderAndShadow: yes",
            "",
            "[V4+ Styles]",
            "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, "
            "Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, "
            "Alignment, MarginL, MarginR, MarginV, Encoding",
            f"Style: ReelCaption,{font_name},{font_size},&H0000E5FF,&H00FFFFFF,&H00121218,&H80000000,"
            f"-1,0,0,0,100,100,0,0,1,6,2,2,60,60,{margin_v},1",
            "",
            "[Events]",
            "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text",
        ]
    )

    events: List[str] = []
    word_buffer: List[Dict[str, Any]] = []

    def flush_chunk(chunk: List[Dict[str, Any]]) -> None:
        if not chunk:
            return
        start = float(chunk[0]["start"])
        end = float(chunk[-1]["end"])
        if end <= start:
            end = start + 0.15
        karaoke_parts: List[str] = []
        for idx, item in enumerate(chunk):
            w_start = float(item["start"])
            w_end = float(item["end"])
            next_start = float(chunk[idx + 1]["start"]) if idx + 1 < len(chunk) else w_end
            dur_cs = max(8, int(round(max(w_end, next_start) - w_start) * 100))
            clean_word = escape_ass_text(str(item["word"]).strip())
            if clean_word:
                karaoke_parts.append(f"{{\\k{dur_cs}}}{clean_word}")
        if karaoke_parts:
            line = (
                f"Dialogue: 0,{format_ass_timestamp(start)},{format_ass_timestamp(end)},"
                f"ReelCaption,,0,0,0,,{' '.join(karaoke_parts)}"
            )
            events.append(line)

    for seg in segments:
        words = seg.get("words") or []
        if words:
            for w in words:
                txt = str(w.get("word", "")).strip()
                if not txt:
                    continue
                word_buffer.append(
                    {
                        "word": txt,
                        "start": float(w.get("start", seg.get("start", 0.0))),
                        "end": float(w.get("end", seg.get("end", 0.0))),
                    }
                )
                if len(word_buffer) >= max_words_per_cue or txt.endswith((".", "!", "?", ",")):
                    flush_chunk(word_buffer)
                    word_buffer = []
        else:
            text = str(seg.get("text", "")).strip()
            if text:
                s = float(seg.get("start", 0.0))
                e = max(s + 0.2, float(seg.get("end", s + 1.0)))
                events.append(
                    f"Dialogue: 0,{format_ass_timestamp(s)},{format_ass_timestamp(e)},"
                    f"ReelCaption,,0,0,0,,{escape_ass_text(text)}"
                )

    flush_chunk(word_buffer)
    return header + "\n" + "\n".join(events) + "\n"


def build_edit_plan(
    source_path: str,
    segments: List[TimeSegment],
    aspect: str = "preserve",
    normalize_audio: bool = True,
    subtitles_path: Optional[str] = None,
    bgm_path: Optional[str] = None,
    bgm_volume: float = 0.12,
) -> Dict[str, Any]:
    """Crea la estructura declarativa edit.json que consume render-edit."""
    if not segments:
        raise ValueError("El plan de edición necesita al menos un segmento temporal válido.")
    if aspect not in {"preserve", "vertical", "horizontal", "square"}:
        raise ValueError(f"Relación de aspecto no soportada: {aspect}")

    return {
        "version": 1,
        "source": source_path,
        "aspect": aspect,
        "normalize_audio_lufs": -14.0 if normalize_audio else None,
        "subtitles": subtitles_path,
        "bgm": {"path": bgm_path, "volume": round(max(0.0, min(1.0, bgm_volume)), 3)} if bgm_path else None,
        "segments": [asdict(seg) for seg in segments],
    }


def compute_target_dimensions(aspect: str, src_w: int, src_h: int) -> Tuple[int, int]:
    """Resuelve resolución par de salida según formato destino."""
    if aspect == "vertical":
        return (1080, 1920)
    if aspect == "horizontal":
        return (1920, 1080)
    if aspect == "square":
        return (1080, 1080)
    even_w = max(2, src_w - (src_w % 2))
    even_h = max(2, src_h - (src_h % 2))
    return (even_w, even_h)
