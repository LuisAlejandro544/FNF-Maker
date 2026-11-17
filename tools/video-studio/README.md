# Video Studio CLI (Herramientas ejecutables para descargar y editar videos con IA)

Conjunto modular de ejecutables portables con licencias 100% permisivas (`Unlicense`, `MIT`, `BSD-3-Clause`, `Apache-2.0`) pensado para que cualquier agente de IA pueda instalar todo el entorno en minutos y editar desde archivos cortos hasta `.mp4` largos y pesados (varios GB) sin saturar la memoria RAM ni gastar contexto innecesario.

## 1. Instalación rápida en un entorno nuevo

```bash
./tools/video-studio/setup-env.sh
```

Instala y configura automáticamente:
- `yt-dlp` (última versión oficial) + ejecutable `yt-dlp-latest`
- `deno` + enlace a `node` y `ejs:github` para desafíos JavaScript
- `uv` + entorno `/opt/video-studio-venv` con `faster-whisper` (incluye Silero VAD), `scenedetect`, `opencv-python-headless`, `pillow` y `av`
- Validación de `ffmpeg` / `ffprobe`

## 2. Flujo recomendado para `.mp4` largos y pesados

1. **Ingesta en streaming (sin cargar todo el `.mp4` en RAM)**:
   ```bash
   ./tools/video-studio/ingest-long-video ruta/al/video_largo.mp4 --aspect vertical
   ```
   Genera en `tools/video-studio/workspace/`:
   - `proxies/*_proxy480p.mp4` (proxy ligero para inspección rápida)
   - `data/*/audio_16k.wav` (audio mono 16 kHz para transcripción)
   - `data/*/silences.json` (silencios detectados + segmentos activos)
   - `data/*/*-Scenes.csv` (cortes visuales de escena)
   - `edits/*_auto_edit.json` (plan declarativo inicial listo para editar)

2. **Transcripción con Silero VAD y marcas palabra por palabra**:
   ```bash
   ./tools/video-studio/transcribe-video tools/video-studio/workspace/data/<nombre>/audio_16k.wav --model base --vertical
   ```
   Exporta `transcript.json`, `subtitles.srt` y `subtitles.ass` (estilo karaoke dinámico).

3. **Renderizado desde `edit.json`**:
   - **Corte rápido sin recodificar** (ideal para recortar o dividir un `.mp4` gigante en segundos):
     ```bash
     ./tools/video-studio/render-edit tools/video-studio/workspace/edits/<plan>.json --mode copy
     ```
   - **Borrador rápido 480p** (con verificación visual `--qc`):
     ```bash
     ./tools/video-studio/render-edit tools/video-studio/workspace/edits/<plan>.json --mode draft --qc
     ```
   - **Exportación maestra 1080p / `9:16`** (normalización `-14 LUFS`, música opcional y subtítulos quemados):
     ```bash
     ./tools/video-studio/render-edit tools/video-studio/workspace/edits/<plan>.json --mode master --qc
     ```

4. **Atajo rápido para sacar un Reel / Short `9:16` de un tramo**:
   ```bash
   ./tools/video-studio/make-vertical-reel video_largo.mp4 --start 125.0 --end 168.5 --subtitles
   ```

5. **Descarga de videos desde consola**:
   ```bash
   ./tools/video-studio/download-video "https://..."
   # Si la plataforma bloquea IPs de nube, pasa cookies Netscape:
   ./tools/video-studio/download-video --cookies cookies.txt "https://..."
   ```

6. **Generador de combate stickman animado (Azul vs Rojo, 20s con escenario y audio)**:
   ```bash
   ./tools/video-studio/render-stickman-fight --duration 20 --fps 30
   ```

## 3. Ejecutar pruebas unitarias del pipeline

```bash
/opt/video-studio-venv/bin/python -m unittest tools/video-studio/test_video_pipeline.py
```
