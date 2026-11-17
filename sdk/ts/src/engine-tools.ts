/**
 * Archivo: sdk/ts/src/engine-tools.ts
 * Propósito: Herramientas modulares en TypeScript para validar y serializar
 * `chart.json` completo, calcular el estado de cuenta regresiva BPM ("3, 2, 1, ¡YA!"),
 * inspeccionar cabeceras `.astc`, detectar formatos `.json` de spritesheets/Animate,
 * probar el algoritmo de auto-armado Sparrow y validar audio/hot-compile.
 */

export type NoteDirection = 'left' | 'down' | 'up' | 'right';
export type EntityRole = 'background' | 'player' | 'opponent' | 'decoration';

export type SpriteJsonFormat =
  | 'texture_packer_hash'
  | 'texture_packer_array'
  | 'animate_spritemap'
  | 'animate_animation';

export interface ChartEventSpec {
  timeMs: number;
  name: string;
  value1: string;
  value2: string;
}

export interface ChartSectionSpec {
  mustHitSection: boolean;
  sectionBeats: number;
  bpm: number;
  changeBpm: boolean;
  altAnim: boolean;
  gfSection: boolean;
}

export interface ParsedChartNote {
  timeMs: number;
  direction: NoteDirection;
  isPlayer: boolean;
  sustainMs: number;
}

export interface ParsedChart {
  title: string;
  bpm: number;
  speed: number;
  needsVoices: boolean;
  player1: string;
  player2: string;
  gfVersion: string;
  stage: string;
  arrowSkin: string;
  splashSkin: string;
  validScore: boolean;
  events: ChartEventSpec[];
  sections: ChartSectionSpec[];
  notes: ParsedChartNote[];
}

export interface CountdownState {
  active: boolean;
  stage: number;
  label: '' | '3' | '2' | '1' | '¡YA!';
  progress: number;
  beatMs: number;
}

export interface AstcHeaderInfo {
  blockDimX: number;
  blockDimY: number;
  blockDimZ: number;
  width: number;
  height: number;
  depth: number;
  expectedPayloadBytes: number;
  totalExpectedBytes: number;
}

export interface SparrowBindings {
  idle?: string;
  left?: string;
  down?: string;
  up?: string;
  right?: string;
}

export interface StageEntitySpec {
  id: string;
  role: EntityRole;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  visible: boolean;
  zIndex: number;
}

const DIRECTIONS: NoteDirection[] = ['left', 'down', 'up', 'right'];

export function parseChartJson(rawJson: string): ParsedChart {
  const root = JSON.parse(rawJson);
  const song = root?.song && typeof root.song === 'object' ? root.song : root;
  if (!song || !Array.isArray(song.notes)) {
    throw new Error('Chart JSON inválido: falta arreglo notes');
  }

  const bpm = typeof song.bpm === 'number' && song.bpm > 10 ? song.bpm : 120;
  const speed = typeof song.speed === 'number' && song.speed > 0.1 ? song.speed : 1.8;
  const title = typeof song.song === 'string' ? song.song : 'Sin título';
  const needsVoices = typeof song.needsVoices === 'boolean' ? song.needsVoices : true;
  const player1 = typeof song.player1 === 'string' ? song.player1 : 'protagonista';
  const player2 = typeof song.player2 === 'string' ? song.player2 : 'rival';
  const gfVersion = typeof song.gfVersion === 'string' ? song.gfVersion : 'acompanante';
  const stage = typeof song.stage === 'string' ? song.stage : 'escenario_base';
  const arrowSkin = typeof song.arrowSkin === 'string' ? song.arrowSkin : '';
  const splashSkin = typeof song.splashSkin === 'string' ? song.splashSkin : '';
  const validScore = typeof song.validScore === 'boolean' ? song.validScore : true;

  const events: ChartEventSpec[] = [];
  if (Array.isArray(song.events)) {
    for (const group of song.events) {
      if (!Array.isArray(group) || group.length < 2) continue;
      const timeMs = Math.max(0, Number(group[0]) || 0);
      const inner = group[1];
      if (Array.isArray(inner)) {
        for (const ev of inner) {
          if (!Array.isArray(ev) || typeof ev[0] !== 'string') continue;
          events.push({
            timeMs,
            name: ev[0],
            value1: ev[1] !== undefined ? String(ev[1]) : '',
            value2: ev[2] !== undefined ? String(ev[2]) : '',
          });
        }
      }
    }
  }

  const notes: ParsedChartNote[] = [];
  const sections: ChartSectionSpec[] = [];
  let runningBpm = bpm;

  for (const sec of song.notes) {
    const mustHit = Boolean(sec?.mustHitSection ?? true);
    const changeBpm = Boolean(sec?.changeBPM ?? false);
    const secBpm = typeof sec?.bpm === 'number' && sec.bpm > 10 ? sec.bpm : runningBpm;
    if (changeBpm) {
      runningBpm = secBpm;
    }
    const sectionBeats =
      typeof sec?.sectionBeats === 'number' && sec.sectionBeats > 0
        ? sec.sectionBeats
        : typeof sec?.lengthInSteps === 'number' && sec.lengthInSteps > 0
          ? sec.lengthInSteps / 4
          : 4;

    sections.push({
      mustHitSection: mustHit,
      sectionBeats,
      bpm: secBpm,
      changeBpm,
      altAnim: Boolean(sec?.altAnim ?? false),
      gfSection: Boolean(sec?.gfSection ?? false),
    });

    const secNotes = Array.isArray(sec?.sectionNotes) ? sec.sectionNotes : [];
    for (const item of secNotes) {
      if (!Array.isArray(item) || item.length < 2) continue;
      const timeMs = Math.max(0, Number(item[0]) || 0);
      const rawLane = Number(item[1]);
      if (!Number.isFinite(rawLane)) continue;

      // Eventos inline clásicos con carril -1
      if (rawLane < 0) {
        if (typeof item[2] === 'string') {
          events.push({
            timeMs,
            name: item[2],
            value1: item[3] !== undefined ? String(item[3]) : '',
            value2: item[4] !== undefined ? String(item[4]) : '',
          });
        }
        continue;
      }

      const lane = Math.floor(rawLane) % 8;
      const sustainMs = Math.max(0, Number(item[2]) || 0);
      const isFirstGroup = lane < 4;
      const isPlayer = mustHit ? isFirstGroup : !isFirstGroup;
      notes.push({
        timeMs,
        direction: DIRECTIONS[lane % 4],
        isPlayer,
        sustainMs,
      });
    }
  }

  notes.sort((a, b) => a.timeMs - b.timeMs);
  events.sort((a, b) => a.timeMs - b.timeMs);
  return {
    title,
    bpm,
    speed,
    needsVoices,
    player1,
    player2,
    gfVersion,
    stage,
    arrowSkin,
    splashSkin,
    validScore,
    events,
    sections,
    notes,
  };
}

export function serializeChartJson(chart: ParsedChart): string {
  let cursorMs = 0;
  let activeBpm = chart.bpm > 10 ? chart.bpm : 120;

  const sectionsMeta =
    chart.sections.length > 0
      ? chart.sections
      : [
          {
            mustHitSection: true,
            sectionBeats: 4,
            bpm: activeBpm,
            changeBpm: false,
            altAnim: false,
            gfSection: false,
          },
        ];

  const sectionWindows = sectionsMeta.map((sec) => {
    if (sec.changeBpm && sec.bpm > 10) {
      activeBpm = sec.bpm;
    }
    const beats = sec.sectionBeats > 0 ? sec.sectionBeats : 4;
    const durationMs = beats * (60000 / activeBpm);
    const startMs = cursorMs;
    cursorMs += durationMs;
    return { sec, startMs, endMs: cursorMs, sectionNotes: [] as Array<[number, number, number]> };
  });

  for (const note of chart.notes) {
    let idx = sectionWindows.findIndex((w) => note.timeMs < w.endMs - 0.5);
    if (idx === -1) idx = sectionWindows.length - 1;
    const win = sectionWindows[idx];
    const dirIdx = DIRECTIONS.indexOf(note.direction);
    const laneOffset = win.sec.mustHitSection
      ? note.isPlayer
        ? 0
        : 4
      : note.isPlayer
        ? 4
        : 0;
    win.sectionNotes.push([note.timeMs, laneOffset + dirIdx, note.sustainMs]);
  }

  const groupedEvents: Array<[number, Array<[string, string, string]>]> = [];
  for (const ev of chart.events) {
    const last = groupedEvents[groupedEvents.length - 1];
    if (last && Math.abs(last[0] - ev.timeMs) < 0.01) {
      last[1].push([ev.name, ev.value1, ev.value2]);
    } else {
      groupedEvents.push([ev.timeMs, [[ev.name, ev.value1, ev.value2]]]);
    }
  }

  return JSON.stringify(
    {
      song: {
        song: chart.title,
        bpm: chart.bpm,
        speed: chart.speed,
        needsVoices: chart.needsVoices,
        player1: chart.player1,
        player2: chart.player2,
        gfVersion: chart.gfVersion,
        stage: chart.stage,
        arrowSkin: chart.arrowSkin,
        splashSkin: chart.splashSkin,
        validScore: chart.validScore,
        events: groupedEvents,
        notes: sectionWindows.map((w) => ({
          sectionNotes: w.sectionNotes,
          sectionBeats: w.sec.sectionBeats,
          lengthInSteps: Math.round(w.sec.sectionBeats * 4),
          mustHitSection: w.sec.mustHitSection,
          bpm: w.sec.bpm,
          changeBPM: w.sec.changeBpm,
          altAnim: w.sec.altAnim,
          gfSection: w.sec.gfSection,
        })),
      },
    },
    null,
    2,
  );
}

export function computeCountdownState(songPosMs: number, bpm: number): CountdownState {
  const safeBpm = Math.max(20, Math.min(400, bpm || 120));
  const beatMs = 60000 / safeBpm;
  if (songPosMs >= 0) {
    return { active: false, stage: -1, label: '', progress: 1, beatMs };
  }
  const elapsedMs = Math.max(0, 4 * beatMs + songPosMs);
  const stage = Math.min(3, Math.max(0, Math.floor(elapsedMs / beatMs)));
  const progress = Math.min(1, Math.max(0, (elapsedMs - stage * beatMs) / beatMs));
  const labels: Array<'3' | '2' | '1' | '¡YA!'> = ['3', '2', '1', '¡YA!'];
  return {
    active: true,
    stage,
    label: labels[stage],
    progress,
    beatMs,
  };
}

export function parseAstcHeader(bytes: Uint8Array): AstcHeaderInfo {
  if (bytes.length < 16) {
    throw new Error('Archivo ASTC truncado: faltan los 16 bytes de cabecera');
  }
  if (
    bytes[0] !== 0x13 ||
    bytes[1] !== 0xab ||
    bytes[2] !== 0xa1 ||
    bytes[3] !== 0x5c
  ) {
    throw new Error('Cabecera mágica ASTC inválida');
  }
  const blockDimX = bytes[4];
  const blockDimY = bytes[5];
  const blockDimZ = bytes[6];
  if (blockDimX < 1 || blockDimX > 12 || blockDimY < 1 || blockDimY > 12 || blockDimZ !== 1) {
    throw new Error(`Tamaño de bloque ASTC no soportado (${blockDimX}x${blockDimY}x${blockDimZ})`);
  }
  const width = bytes[7] | (bytes[8] << 8) | (bytes[9] << 16);
  const height = bytes[10] | (bytes[11] << 8) | (bytes[12] << 16);
  const depth = bytes[13] | (bytes[14] << 8) | (bytes[15] << 16);
  if (width === 0 || height === 0 || depth === 0) {
    throw new Error('Dimensiones ASTC inválidas (0)');
  }
  const blocksX = Math.ceil(width / blockDimX);
  const blocksY = Math.ceil(height / blockDimY);
  const expectedPayloadBytes = blocksX * blocksY * depth * 16;
  return {
    blockDimX,
    blockDimY,
    blockDimZ,
    width,
    height,
    depth,
    expectedPayloadBytes,
    totalExpectedBytes: 16 + expectedPayloadBytes,
  };
}

export function detectSpriteJsonFormat(rawJson: string): SpriteJsonFormat {
  const parsed = JSON.parse(rawJson);
  if (parsed?.ATLAS?.SPRITES && Array.isArray(parsed.ATLAS.SPRITES)) {
    return 'animate_spritemap';
  }
  if ((parsed?.AN || parsed?.ANIMATION) && (parsed?.SD || parsed?.AN?.TL || parsed?.ANIMATION?.TIMELINE)) {
    return 'animate_animation';
  }
  if (parsed?.frames && typeof parsed.frames === 'object') {
    return Array.isArray(parsed.frames) ? 'texture_packer_array' : 'texture_packer_hash';
  }
  throw new Error('JSON de sprites no reconocido (esperado TexturePacker, spritemap1.json o Animation.json)');
}

export interface TrimmedFramePlacement {
  visualWidth: number;
  visualHeight: number;
  logicalWidth: number;
  logicalHeight: number;
  offsetX: number;
  offsetY: number;
}

export function computeTrimmedFramePlacement(frame: {
  width: number;
  height: number;
  frameX: number;
  frameY: number;
  frameWidth: number;
  frameHeight: number;
  rotated: boolean;
}): TrimmedFramePlacement {
  const visualWidth = frame.rotated ? frame.height : frame.width;
  const visualHeight = frame.rotated ? frame.width : frame.height;
  return {
    visualWidth,
    visualHeight,
    logicalWidth: Math.max(frame.frameWidth, visualWidth, 1),
    logicalHeight: Math.max(frame.frameHeight, visualHeight, 1),
    offsetX: -frame.frameX,
    offsetY: -frame.frameY,
  };
}

export function autoBindSparrowPrefixes(prefixes: string[]): SparrowBindings {
  const bindings: SparrowBindings = {};
  for (const prefix of prefixes) {
    const low = prefix.toLowerCase();
    if (low.includes('miss') || low.includes('fallo')) continue;
    if (!bindings.idle && (low.includes('idle') || low.includes('dance') || low.includes('reposo'))) {
      bindings.idle = prefix;
    } else if (!bindings.left && (low.includes('left') || low.includes('izq'))) {
      bindings.left = prefix;
    } else if (!bindings.down && (low.includes('down') || low.includes('abajo'))) {
      bindings.down = prefix;
    } else if (!bindings.up && (low.includes('up') || low.includes('arriba'))) {
      bindings.up = prefix;
    } else if (!bindings.right && (low.includes('right') || low.includes('der'))) {
      bindings.right = prefix;
    }
  }
  if (!bindings.idle && prefixes.length > 0) {
    bindings.idle = prefixes[0];
  }
  return bindings;
}

export function validateAudioTrackCount(paths: Array<string | null | undefined>): number {
  const active = paths.filter((p) => typeof p === 'string' && p.trim().length > 0);
  if (active.length > 3) {
    throw new Error('Se permiten como máximo 3 archivos de música (Mix/Inst, Player, Rival)');
  }
  return active.length;
}

export function shouldDiscardInvisibleEntity(
  entity: StageEntitySpec,
  scriptSources: string[],
  virtualWidth = 1280,
  virtualHeight = 720,
): boolean {
  const touchedByScript = scriptSources.some((src) => src.includes(entity.id));
  if (touchedByScript) return false;
  if (!entity.visible || entity.opacity <= 0.005 || entity.scale <= 0.005) return true;

  const halfW = (entity.width * entity.scale) * 0.5;
  const halfH = (entity.height * entity.scale) * 0.5;
  const margin = 450;
  return (
    entity.x + halfW < -margin ||
    entity.x - halfW > virtualWidth + margin ||
    entity.y + halfH < -margin ||
    entity.y - halfH > virtualHeight + margin
  );
}

export function stripTrailingFrameDigits(rawName: string): string {
  const withoutExt = rawName.trim().replace(/\.(png|jpg|jpeg|webp|astc)$/i, '');
  return withoutExt.replace(/\d+$/, '').trim();
}

export function compareNaturalFrameNames(a: string, b: string): number {
  const pa = stripTrailingFrameDigits(a);
  const pb = stripTrailingFrameDigits(b);
  if (pa !== pb) return pa.localeCompare(pb);
  const na = Number(a.replace(/\.(png|jpg|jpeg|webp|astc)$/i, '').slice(pa.length) || '0');
  const nb = Number(b.replace(/\.(png|jpg|jpeg|webp|astc)$/i, '').slice(pb.length) || '0');
  return na !== nb ? na - nb : a.localeCompare(b);
}

export function mapRotatedSpriteSample(
  lx: number,
  ly: number,
  rectW: number,
  rectH: number,
  rotated: boolean,
): { u: number; v: number } {
  const maxU = Math.max(0, rectW - 1);
  const maxV = Math.max(0, rectH - 1);
  if (rotated) {
    // +90° CW empacado -> inversa u = (w - 1) - (ly - 0.5), v = lx - 0.5
    return {
      u: Math.min(maxU, Math.max(0, maxU - (ly - 0.5))),
      v: Math.min(maxV, Math.max(0, lx - 0.5)),
    };
  }
  return {
    u: Math.min(maxU, Math.max(0, lx - 0.5)),
    v: Math.min(maxV, Math.max(0, ly - 0.5)),
  };
}

export interface PoseInput {
  signature: string;
  width: number;
  height: number;
}

export interface PackedBoundedAtlasResult {
  uniqueCount: number;
  sheetWidth: number;
  sheetHeight: number;
  worldScale: number;
  frameToPoseIndex: number[];
}

export function packPosesIntoBoundedAtlas(
  poses: PoseInput[],
  maxAtlasDim = 4096,
  pad = 2,
): PackedBoundedAtlasResult {
  const unique: Array<{ width: number; height: number }> = [];
  const sigMap = new Map<string, number>();
  const frameToPoseIndex: number[] = [];

  let maxSide = 1;
  for (const p of poses) {
    let idx = sigMap.get(p.signature);
    if (idx === undefined) {
      idx = unique.length;
      sigMap.set(p.signature, idx);
      unique.push({ width: Math.max(1, p.width), height: Math.max(1, p.height) });
      maxSide = Math.max(maxSide, p.width, p.height);
    }
    frameToPoseIndex.push(idx);
  }

  let worldScale = Math.min(1, 640 / maxSide);
  let sheetWidth = 64;
  let sheetHeight = 64;

  for (let attempt = 0; attempt < 5; attempt++) {
    const sized = unique
      .map((u, i) => ({
        i,
        w: Math.max(4, Math.ceil(u.width * worldScale)),
        h: Math.max(4, Math.ceil(u.height * worldScale)),
      }))
      .sort((a, b) => b.h - a.h || b.w - a.w);

    let curX = pad;
    let curY = pad;
    let rowH = 0;
    let sw = 64;
    for (const item of sized) {
      if (curX + item.w + pad > maxAtlasDim && curX > pad) {
        curX = pad;
        curY += rowH + pad;
        rowH = 0;
      }
      sw = Math.max(sw, curX + item.w + pad);
      curX += item.w + pad;
      rowH = Math.max(rowH, item.h);
    }
    const sh = Math.max(64, curY + rowH + pad);
    sheetWidth = sw;
    sheetHeight = sh;
    if (sh <= 3840) break;
    const shrink = Math.min(0.92, Math.max(0.35, Math.sqrt(3600 / sh) * 0.94));
    worldScale = Math.max(0.22, worldScale * shrink);
  }

  return {
    uniqueCount: unique.length,
    sheetWidth,
    sheetHeight,
    worldScale,
    frameToPoseIndex,
  };
}

export function computeUniformCharacterDrawBox(
  entityBox: { cx: number; top: number; w: number; h: number },
  frame: { width: number; height: number; frameWidth: number; frameHeight: number; rotated: boolean },
): { boxLeft: number; boxTop: number; boxW: number; boxH: number; fit: number } {
  const visW = frame.rotated ? frame.height : frame.width;
  const visH = frame.rotated ? frame.width : frame.height;
  const logicalW = Math.max(1, frame.frameWidth, visW);
  const logicalH = Math.max(1, frame.frameHeight, visH);
  const fit = Math.min(entityBox.w / logicalW, entityBox.h / logicalH);
  const boxW = logicalW * fit;
  const boxH = logicalH * fit;
  return {
    boxLeft: entityBox.cx - boxW / 2,
    boxTop: entityBox.top + entityBox.h - boxH,
    boxW,
    boxH,
    fit,
  };
}

