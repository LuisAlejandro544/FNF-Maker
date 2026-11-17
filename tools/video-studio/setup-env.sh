#!/usr/bin/env bash
# ============================================================================
# Archivo: tools/video-studio/setup-env.sh
# Propósito: Instala o actualiza en cualquier entorno Linux x86_64/arm64 todas
#            las herramientas ejecutables necesarias para descargar y editar
#            videos cortos o .mp4 largos/pesados con IA desde terminal:
#            - yt-dlp (última versión, Unlicense) + yt-dlp-latest
#            - Deno (MIT) + configuración Node/EJS para desafíos JavaScript
#            - uv (MIT/Apache-2.0) + entorno Python con faster-whisper (MIT,
#              incluye Silero VAD), PySceneDetect (BSD-3) y OpenCV headless
#            - Verifica ffmpeg / ffprobe (o instala binario estático si falta)
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="${VIDEO_STUDIO_VENV:-/opt/video-studio-venv}"
BIN_DIR="${VIDEO_STUDIO_BIN_DIR:-/usr/local/bin}"

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64)
    YTDLP_ASSET="yt-dlp_linux"
    DENO_ASSET="deno-x86_64-unknown-linux-gnu.zip"
    ;;
  aarch64|arm64)
    YTDLP_ASSET="yt-dlp_linux_aarch64"
    DENO_ASSET="deno-aarch64-unknown-linux-gnu.zip"
    ;;
  *)
    echo "[setup-env] Arquitectura no soportada automáticamente: $ARCH_RAW" >&2
    exit 1
    ;;
esac

mkdir -p "$BIN_DIR" "$SCRIPT_DIR/workspace/raw" "$SCRIPT_DIR/workspace/proxies" "$SCRIPT_DIR/workspace/data" "$SCRIPT_DIR/workspace/edits" "$SCRIPT_DIR/workspace/exports"

echo "[1/5] Instalando/actualizando yt-dlp (última versión oficial)..."
curl -fsSL "https://github.com/yt-dlp/yt-dlp/releases/latest/download/${YTDLP_ASSET}" -o "${BIN_DIR}/yt-dlp"
chmod +x "${BIN_DIR}/yt-dlp"

echo "[2/5] Instalando Deno (runtime JavaScript recomendado por yt-dlp/EJS)..."
TMP_ZIP="$(mktemp /tmp/deno-XXXXXX.zip)"
curl -fsSL "https://github.com/denoland/deno/releases/latest/download/${DENO_ASSET}" -o "$TMP_ZIP"
python3 -c "import zipfile; zipfile.ZipFile('$TMP_ZIP').extract('deno', '$BIN_DIR')"
rm -f "$TMP_ZIP"
chmod +x "${BIN_DIR}/deno"

echo "[3/5] Configurando yt-dlp con Deno + Node + EJS..."
mkdir -p "${HOME}/.config/yt-dlp"
{
  echo "--js-runtimes deno:${BIN_DIR}/deno"
  if command -v node >/dev/null 2>&1; then
    echo "--js-runtimes node:$(command -v node)"
  fi
  echo "--remote-components ejs:github"
} > "${HOME}/.config/yt-dlp/config"

cat << EOF > "${BIN_DIR}/yt-dlp-latest"
#!/usr/bin/env bash
set -euo pipefail
${BIN_DIR}/yt-dlp -U >/dev/null 2>&1 || true
exec ${BIN_DIR}/yt-dlp "\$@"
EOF
chmod +x "${BIN_DIR}/yt-dlp-latest"

echo "[4/5] Instalando uv y entorno Python de análisis/edición ($VENV_DIR)..."
if ! command -v uv >/dev/null 2>&1; then
  curl -fsSL https://astral.sh/uv/install.sh | env UV_INSTALL_DIR="$BIN_DIR" sh
fi

"${BIN_DIR}/uv" venv "$VENV_DIR" --python 3.11 --allow-existing
"${BIN_DIR}/uv" pip install --python "${VENV_DIR}/bin/python" \
  faster-whisper scenedetect opencv-python-headless av pillow

echo "[5/5] Verificando ffmpeg/ffprobe y permisos de ejecutables del proyecto..."
if ! command -v ffmpeg >/dev/null 2>&1 || ! command -v ffprobe >/dev/null 2>&1; then
  echo "[setup-env] ffmpeg/ffprobe no detectados en PATH; instala ffmpeg con apt/apk/brew." >&2
  exit 1
fi

chmod +x "$SCRIPT_DIR/setup-env.sh" \
  "$SCRIPT_DIR/download-video" \
  "$SCRIPT_DIR/ingest-long-video" \
  "$SCRIPT_DIR/transcribe-video" \
  "$SCRIPT_DIR/render-edit" \
  "$SCRIPT_DIR/make-vertical-reel" \
  "$SCRIPT_DIR/render-stickman-fight"

echo ""
echo "=== Entorno Video Studio listo ==="
echo "yt-dlp : $("${BIN_DIR}/yt-dlp" --version)"
echo "deno   : $("${BIN_DIR}/deno" --version | head -n 1)"
echo "ffmpeg : $(ffmpeg -version | head -n 1)"
echo "python : $("${VENV_DIR}/bin/python" -c 'import faster_whisper, scenedetect, cv2, PIL; print("faster-whisper + scenedetect", scenedetect.__version__, "+ opencv", cv2.__version__, "+ pillow", PIL.__version__)')"
