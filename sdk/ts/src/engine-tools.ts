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
  missLeft?: string;
  missDown?: string;
  missUp?: string;
  missRight?: string;
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

function unwrapChartContainer(root: Record<string, unknown>): Record<string, unknown> {
  let curr: Record<string, unknown> = root;
  for (let i = 0; i < 4; i++) {
    if (curr.notes !== undefined || curr.sections !== undefined || curr.strumLines !== undefined) {
      break;
    }
    const candidate =
      (curr.song && typeof curr.song === 'object' && !Array.isArray(curr.song)
        ? (curr.song as Record<string, unknown>)
        : null) ??
      (curr.chart && typeof curr.chart === 'object' && !Array.isArray(curr.chart)
        ? (curr.chart as Record<string, unknown>)
        : null) ??
      (curr.data && typeof curr.data === 'object' && !Array.isArray(curr.data)
        ? (curr.data as Record<string, unknown>)
        : null);
    if (!candidate) break;
    curr = candidate;
  }
  return curr;
}

function asFiniteNumber(val: unknown): number | null {
  if (typeof val === 'number' && Number.isFinite(val)) return val;
  if (typeof val === 'string' && val.trim().length > 0) {
    const n = Number(val.trim());
    if (Number.isFinite(n)) return n;
  }
  return null;
}

function firstValidNumber(candidates: unknown[], minExclusive: number): number | null {
  for (const c of candidates) {
    const n = asFiniteNumber(c);
    if (n !== null && n > minExclusive) return n;
  }
  return null;
}

function firstNonEmptyString(candidates: unknown[]): string | null {
  for (const c of candidates) {
    if (typeof c === 'string' && c.trim().length > 0) return c.trim();
  }
  return null;
}

function extractScrollSpeed(obj: Record<string, unknown>): number | null {
  for (const key of ['speed', 'scrollSpeed']) {
    const val = obj[key];
    const direct = asFiniteNumber(val);
    if (direct !== null && direct > 0.1) return direct;
    if (val && typeof val === 'object' && !Array.isArray(val)) {
      const map = val as Record<string, unknown>;
      for (const diff of ['hard', 'normal', 'easy', 'erect', 'nightmare']) {
        const n = asFiniteNumber(map[diff]);
        if (n !== null && n > 0.1) return n;
      }
      for (const anyVal of Object.values(map)) {
        const n = asFiniteNumber(anyVal);
        if (n !== null && n > 0.1) return n;
      }
    }
  }
  return null;
}

function pickVSliceDifficultyArray(notesMap: Record<string, unknown>): unknown[] | null {
  for (const diff of ['hard', 'normal', 'easy', 'erect', 'nightmare']) {
    const arr = notesMap[diff];
    if (Array.isArray(arr) && arr.length > 0) return arr;
  }
  for (const val of Object.values(notesMap)) {
    if (Array.isArray(val) && val.length > 0) return val;
  }
  for (const val of Object.values(notesMap)) {
    if (Array.isArray(val)) return val;
  }
  return null;
}

function buildSyntheticSections(
  bpm: number,
  notes: ParsedChartNote[],
  events: ChartEventSpec[],
): ChartSectionSpec[] {
  const safeBpm = Math.max(40, bpm);
  const secMs = ((60000 / safeBpm) / 4) * 16;
  let maxMs = secMs * 4;
  for (const n of notes) maxMs = Math.max(maxMs, n.timeMs + n.sustainMs);
  for (const ev of events) maxMs = Math.max(maxMs, ev.timeMs);
  const totalSecs = Math.min(2000, Math.max(4, Math.ceil(maxMs / secMs)));
  const out: ChartSectionSpec[] = [];
  for (let idx = 0; idx < totalSecs; idx++) {
    const startMs = idx * secMs;
    const endMs = startMs + secMs;
    let pCount = 0;
    let oCount = 0;
    for (const n of notes) {
      if (n.timeMs >= startMs - 1 && n.timeMs < endMs - 1) {
        if (n.isPlayer) pCount++;
        else oCount++;
      }
    }
    const mustHit = pCount === 0 && oCount === 0 ? true : pCount >= oCount;
    out.push({
      mustHitSection: mustHit,
      sectionBeats: 4,
      bpm: safeBpm,
      changeBpm: false,
      altAnim: false,
      gfSection: false,
    });
  }
  return out;
}

export function parseChartJson(rawJson: string): ParsedChart {
  const cleaned = rawJson.replace(/^\uFEFF/, '').trim();
  const root = JSON.parse(cleaned) as Record<string, unknown>;
  const song = unwrapChartContainer(root);

  const timeChangesSong = Array.isArray(song.timeChanges) ? song.timeChanges : [];
  const timeChangesRoot = Array.isArray(root.timeChanges) ? root.timeChanges : [];
  const bpm =
    firstValidNumber(
      [
        song.bpm,
        song.BPM,
        root.bpm,
        root.BPM,
        (timeChangesSong[0] as Record<string, unknown> | undefined)?.bpm,
        (timeChangesRoot[0] as Record<string, unknown> | undefined)?.bpm,
      ],
      10,
    ) ?? 120;

  const speed = extractScrollSpeed(song) ?? extractScrollSpeed(root) ?? 1.8;
  const title =
    firstNonEmptyString([
      song.song,
      song.songName,
      song.title,
      song.name,
      root.songName,
      root.song,
      root.title,
    ]) ?? 'Sin título';
  const needsVoices =
    typeof song.needsVoices === 'boolean'
      ? song.needsVoices
      : typeof root.needsVoices === 'boolean'
        ? root.needsVoices
        : true;

  const playData =
    (song.playData as Record<string, unknown> | undefined) ??
    (root.playData as Record<string, unknown> | undefined);
  const playChars = playData?.characters as Record<string, unknown> | undefined;

  const player1 =
    firstNonEmptyString([song.player1, playChars?.player, root.player1]) ?? 'protagonista';
  const player2 =
    firstNonEmptyString([song.player2, playChars?.opponent, root.player2]) ?? 'rival';
  const gfVersion =
    firstNonEmptyString([
      song.gfVersion,
      song.player3,
      playChars?.girlfriend,
      root.gfVersion,
      root.player3,
    ]) ?? 'acompanante';
  const stage =
    firstNonEmptyString([song.stage, playData?.stage, root.stage]) ?? 'escenario_base';
  const arrowSkin = firstNonEmptyString([song.arrowSkin, root.arrowSkin]) ?? '';
  const splashSkin = firstNonEmptyString([song.splashSkin, root.splashSkin]) ?? '';
  const validScore =
    typeof song.validScore === 'boolean'
      ? song.validScore
      : typeof root.validScore === 'boolean'
        ? root.validScore
        : true;

  const events: ChartEventSpec[] = [];
  const rawEvents = Array.isArray(song.events)
    ? song.events
    : Array.isArray(root.events)
      ? root.events
      : [];
  for (const group of rawEvents) {
    if (Array.isArray(group) && group.length >= 2) {
      const timeMs = Math.max(0, asFiniteNumber(group[0]) ?? 0);
      const inner = group[1];
      if (Array.isArray(inner)) {
        for (const ev of inner) {
          if (!Array.isArray(ev) || typeof ev[0] !== 'string' || !ev[0]) continue;
          events.push({
            timeMs,
            name: ev[0],
            value1: ev[1] !== undefined ? String(ev[1]) : '',
            value2: ev[2] !== undefined ? String(ev[2]) : '',
          });
        }
      } else if (typeof inner === 'string' && inner) {
        events.push({
          timeMs,
          name: inner,
          value1: group[2] !== undefined ? String(group[2]) : '',
          value2: group[3] !== undefined ? String(group[3]) : '',
        });
      }
    } else if (group && typeof group === 'object') {
      const evObj = group as Record<string, unknown>;
      const timeMs = Math.max(0, asFiniteNumber(evObj.t ?? evObj.time) ?? 0);
      const name = typeof (evObj.e ?? evObj.name) === 'string' ? String(evObj.e ?? evObj.name).trim() : '';
      if (!name) continue;
      const params = Array.isArray(evObj.params) ? evObj.params : null;
      const value1 = params
        ? params[0] !== undefined
          ? String(params[0])
          : ''
        : evObj.v && typeof evObj.v === 'object'
          ? JSON.stringify(evObj.v)
          : evObj.v !== undefined
            ? String(evObj.v)
            : evObj.value1 !== undefined
              ? String(evObj.value1)
              : '';
      const value2 = params
        ? params[1] !== undefined
          ? String(params[1])
          : ''
        : evObj.value2 !== undefined
          ? String(evObj.value2)
          : '';
      events.push({ timeMs, name, value1, value2 });
    }
  }

  const notes: ParsedChartNote[] = [];
  let sections: ChartSectionSpec[] = [];

  const notesVal = song.notes ?? song.sections ?? root.notes ?? root.sections;
  const strumLines = Array.isArray(song.strumLines)
    ? song.strumLines
    : Array.isArray(root.strumLines)
      ? root.strumLines
      : null;

  const pushFlatNoteObject = (item: Record<string, unknown>, playerFirstGroup: boolean) => {
    const timeMs = Math.max(0, asFiniteNumber(item.t ?? item.time ?? item.strumTime) ?? 0);
    const rawLane = asFiniteNumber(item.d ?? item.lane ?? item.noteData ?? item.id);
    if (rawLane === null || rawLane < 0) return;
    const lane = Math.floor(rawLane) % 8;
    const sustainMs = Math.max(
      0,
      asFiniteNumber(item.l ?? item.sustain ?? item.sustainLength ?? item.sLen) ?? 0,
    );
    const explicitPlayer =
      typeof item.mustPress === 'boolean'
        ? item.mustPress
        : typeof item.isPlayer === 'boolean'
          ? item.isPlayer
          : null;
    const isFirstGroup = lane < 4;
    const isPlayer = explicitPlayer ?? (playerFirstGroup ? isFirstGroup : !isFirstGroup);
    notes.push({
      timeMs,
      direction: DIRECTIONS[lane % 4],
      isPlayer,
      sustainMs,
    });
  };

  if (Array.isArray(notesVal)) {
    const looksLikeSections =
      notesVal.length === 0 ||
      notesVal.some(
        (el) =>
          el &&
          typeof el === 'object' &&
          !Array.isArray(el) &&
          ('sectionNotes' in el ||
            'notes' in el ||
            'mustHitSection' in el ||
            'mustHit' in el ||
            'lengthInSteps' in el ||
            'sectionBeats' in el),
      );

    if (looksLikeSections) {
      let runningBpm = bpm;
      for (const rawSec of notesVal) {
        const sec = (rawSec ?? {}) as Record<string, unknown>;
        const mustHit = Boolean(sec.mustHitSection ?? sec.mustHit ?? true);
        const changeBpm = Boolean(sec.changeBPM ?? false);
        const parsedSecBpm = firstValidNumber([sec.bpm, sec.BPM], 10);
        const secBpm = parsedSecBpm ?? runningBpm;
        if (changeBpm) {
          runningBpm = secBpm;
        }
        const beatsNum = asFiniteNumber(sec.sectionBeats);
        const stepsNum = asFiniteNumber(sec.lengthInSteps);
        const sectionBeats =
          beatsNum !== null && beatsNum > 0
            ? beatsNum
            : stepsNum !== null && stepsNum > 0
              ? stepsNum / 4
              : 4;

        sections.push({
          mustHitSection: mustHit,
          sectionBeats,
          bpm: secBpm,
          changeBpm,
          altAnim: Boolean(sec.altAnim ?? false),
          gfSection: Boolean(sec.gfSection ?? false),
        });

        const secNotes = Array.isArray(sec.sectionNotes)
          ? sec.sectionNotes
          : Array.isArray(sec.notes)
            ? sec.notes
            : [];
        for (const item of secNotes) {
          if (Array.isArray(item) && item.length >= 2) {
            const timeMs = Math.max(0, asFiniteNumber(item[0]) ?? 0);
            const rawLane = asFiniteNumber(item[1]);
            if (rawLane === null) continue;

            // Eventos inline clásicos con carril -1
            if (rawLane < 0) {
              if (typeof item[2] === 'string' && item[2]) {
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
            const sustainMs = Math.max(0, asFiniteNumber(item[2]) ?? 0);
            const isFirstGroup = lane < 4;
            const isPlayer = mustHit ? isFirstGroup : !isFirstGroup;
            notes.push({
              timeMs,
              direction: DIRECTIONS[lane % 4],
              isPlayer,
              sustainMs,
            });
          } else if (item && typeof item === 'object') {
            pushFlatNoteObject(item as Record<string, unknown>, mustHit);
          }
        }
      }
    } else {
      for (const item of notesVal) {
        if (Array.isArray(item) && item.length >= 2) {
          const timeMs = Math.max(0, asFiniteNumber(item[0]) ?? 0);
          const rawLane = asFiniteNumber(item[1]);
          if (rawLane === null || rawLane < 0) continue;
          const lane = Math.floor(rawLane) % 8;
          const sustainMs = Math.max(0, asFiniteNumber(item[2]) ?? 0);
          notes.push({
            timeMs,
            direction: DIRECTIONS[lane % 4],
            isPlayer: lane < 4,
            sustainMs,
          });
        } else if (item && typeof item === 'object') {
          pushFlatNoteObject(item as Record<string, unknown>, true);
        }
      }
      sections = buildSyntheticSections(bpm, notes, events);
    }
  } else if (notesVal && typeof notesVal === 'object' && !Array.isArray(notesVal)) {
    const diffNotes = pickVSliceDifficultyArray(notesVal as Record<string, unknown>);
    if (diffNotes) {
      for (const item of diffNotes) {
        if (item && typeof item === 'object' && !Array.isArray(item)) {
          pushFlatNoteObject(item as Record<string, unknown>, true);
        }
      }
    }
    if (notes.length > 0 || events.length > 0) {
      sections = buildSyntheticSections(bpm, notes, events);
    }
  } else if (strumLines) {
    for (const rawLine of strumLines) {
      if (!rawLine || typeof rawLine !== 'object') continue;
      const line = rawLine as Record<string, unknown>;
      const lineType = asFiniteNumber(line.type);
      const pos = typeof line.position === 'string' ? line.position.toLowerCase() : '';
      const isPlayer =
        lineType === 1
          ? true
          : lineType === 0 || lineType === 2
            ? false
            : pos === 'boyfriend' || pos === 'bf' || pos === 'player';
      const lineNotes = Array.isArray(line.notes) ? line.notes : [];
      for (const rawNote of lineNotes) {
        if (!rawNote || typeof rawNote !== 'object' || Array.isArray(rawNote)) continue;
        const n = rawNote as Record<string, unknown>;
        const timeMs = Math.max(0, asFiniteNumber(n.time ?? n.t) ?? 0);
        const rawLane = asFiniteNumber(n.id ?? n.d ?? n.lane);
        if (rawLane === null || rawLane < 0) continue;
        const sustainMs = Math.max(0, asFiniteNumber(n.sLen ?? n.l ?? n.sustain) ?? 0);
        notes.push({
          timeMs,
          direction: DIRECTIONS[Math.floor(rawLane) % 4],
          isPlayer,
          sustainMs,
        });
      }
    }
    if (notes.length > 0 || events.length > 0 || strumLines.length > 0) {
      sections = buildSyntheticSections(bpm, notes, events);
    }
  } else {
    throw new Error('Chart JSON inválido: falta arreglo notes');
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
    if (low.includes('miss') || low.includes('fallo')) {
      if (!bindings.missLeft && (low.includes('left') || low.includes('izq'))) {
        bindings.missLeft = prefix;
      } else if (!bindings.missDown && (low.includes('down') || low.includes('abajo'))) {
        bindings.missDown = prefix;
      } else if (!bindings.missUp && (low.includes('up') || low.includes('arriba'))) {
        bindings.missUp = prefix;
      } else if (!bindings.missRight && (low.includes('right') || low.includes('der'))) {
        bindings.missRight = prefix;
      }
      continue;
    }
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

export type ScriptLang = 'lua' | 'js' | 'ts';

export interface ScriptLoopOptResult {
  optimizedSource: string;
  discardedLoops: number;
  neutralizedLoops: number;
}

const NECESSARY_ENGINE_APIS = [
  'setEntityProp',
  'getEntityProp',
  'getEntityX',
  'getEntityY',
  'setStrumProp',
  'getStrumProp',
  'tweenEntityProp',
  'tweenStrumProp',
  'tweenCameraPos',
  'tweenCameraZoom',
  'cancelTween',
  'runTimer',
  'cancelTimer',
  'makeGraphic',
  'makeHudGraphic',
  'makeText',
  'setTextString',
  'cloneSprite',
  'removeSprite',
  'setCameraZoom',
  'setCameraBaseZoom',
  'setCameraPos',
  'shakeCamera',
  'playAnim',
  'playEntityAnim',
  'setHealth',
  'addHealth',
  'setScore',
  'addScore',
  'setScrollSpeed',
  'setDownscroll',
  'setHudVisible',
  'getHudVisible',
  'setHudOpacity',
  'getHudOpacity',
  'setHudZoom',
  'getHudZoom',
  'tweenHudProp',
  'tweenHudOpacity',
  'setArrowsVisible',
  'getArrowsVisible',
  'setArrowsOpacity',
  'getArrowsOpacity',
  'tweenArrowsOpacity',
  'setScreenBlackout',
  'getScreenBlackout',
  'tweenScreenBlackout',
  'setTrackVolume',
  'setTrackMute',
  'getSongPosMs',
  'flashCamera',
  'flashHud',
  'flashEffect',
  'preloadEntity',
  'retireEntity',
  'swapEntity',
  'setActiveCharacter',
  'onSongStart',
] as const;

function stripCommentsForApiScan(body: string): string {
  return body
    .replace(/--\[\[[\s\S]*?\]\]/g, '')
    .replace(/--[^\n]*/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/[^\n]*/g, '');
}

function hasNecessaryEngineLogic(body: string): boolean {
  const clean = stripCommentsForApiScan(body);
  return NECESSARY_ENGINE_APIS.some((api) => clean.includes(api));
}

export function optimizeScriptLoops(language: ScriptLang, source: string): ScriptLoopOptResult {
  let discardedLoops = 0;
  let neutralizedLoops = 0;

  if (language === 'lua') {
    const out = source.replace(
      /\b(while\s+(?:true|1)\s+do\b([\s\S]*?)\bend\b|for\s+([^=\n]+=\s*[^,\n]+,\s*)(\d[\d_]*)(?:\s*,\s*[^\n]+)?\s+do\b([\s\S]*?)\bend\b)/g,
      (full, _all, whileBody, forHead, rawLimit, forBody) => {
        if (whileBody !== undefined) {
          if (/\bbreak\b/.test(whileBody)) return full;
          if (!hasNecessaryEngineLogic(whileBody)) {
            discardedLoops += 1;
            return '-- [HotCompile] Bucle pesado Lua sin lógica necesaria descartado';
          }
          neutralizedLoops += 1;
          return `-- [HotCompile] Bucle infinito Lua neutralizado\nfor _bf_once = 1, 1 do${whileBody}end`;
        }
        const limit = Number(String(rawLimit).replace(/_/g, ''));
        const safeCap = /\bonUpdate\b/.test(source) ? 64 : 256;
        if (!Number.isFinite(limit) || limit <= safeCap) return full;
        if (!hasNecessaryEngineLogic(forBody)) {
          discardedLoops += 1;
          return '-- [HotCompile] Bucle for pesado Lua descartado';
        }
        neutralizedLoops += 1;
        return `for ${forHead}${safeCap} do${forBody}end`;
      },
    );
    return { optimizedSource: out, discardedLoops, neutralizedLoops };
  }

  const out = source.replace(
    /\b(while\s*\(\s*(?:true|1|!0|!false)\s*\)\s*\{([\s\S]*?)\}|for\s*\(([^;]*;[^;]*;[^)]*)\)\s*\{([\s\S]*?)\})/g,
    (full, _all, whileBody, forHeader, forBody) => {
      if (whileBody !== undefined) {
        if (/\bbreak\b/.test(whileBody)) return full;
        if (!hasNecessaryEngineLogic(whileBody)) {
          discardedLoops += 1;
          return '/* [HotCompile] Bucle pesado sin lógica necesaria descartado */';
        }
        neutralizedLoops += 1;
        return `/* [HotCompile] Bucle infinito neutralizado */ for (let _bf_once = 0; _bf_once < 1; _bf_once++) {${whileBody}}`;
      }
      const parts = String(forHeader).split(';');
      if (parts.length !== 3) return full;
      const cond = parts[1];
      const match = cond.match(/^(.*?[<>]=?\s*)(\d[\d_]*)\s*$/);
      if (!match) return full;
      const limit = Number(match[2].replace(/_/g, ''));
      const safeCap = /\bonUpdate\b/.test(source) ? 64 : 256;
      if (!Number.isFinite(limit) || limit <= safeCap) return full;
      if (!hasNecessaryEngineLogic(forBody)) {
        discardedLoops += 1;
        return '/* [HotCompile] Bucle pesado sin lógica necesaria descartado */';
      }
      neutralizedLoops += 1;
      return `for (${parts[0]};${match[1]}${safeCap};${parts[2]}) {${forBody}}`;
    },
  );
  return { optimizedSource: out, discardedLoops, neutralizedLoops };
}

export type DeviceResourceTier = 'low_32bit' | 'balanced' | 'high';

export function computeSmartTextureScale(params: {
  origWidth: number;
  origHeight: number;
  entityWidth: number;
  entityHeight: number;
  entityScale: number;
  hasAtlas: boolean;
  maxFrameSide?: number;
  tier: DeviceResourceTier;
}): number {
  const { origWidth, origHeight, entityWidth, entityHeight, entityScale, hasAtlas, maxFrameSide = 0, tier } = params;
  if (origWidth <= 0 || origHeight <= 0) return 1.0;

  const largest = Math.max(origWidth, origHeight);
  const tierCap = hasAtlas
    ? tier === 'low_32bit'
      ? 2560
      : 4096
    : tier === 'low_32bit'
      ? 1536
      : tier === 'balanced'
        ? 2560
        : 4096;

  let targetRatio = largest > tierCap ? tierCap / largest : 1.0;
  const displayMax = Math.max(128, Math.max(entityWidth, entityHeight) * entityScale);

  if (hasAtlas) {
    if (maxFrameSide > 0) {
      const idealFrameMax = Math.max(512, displayMax * 2.2);
      if (maxFrameSide > idealFrameMax * 1.15 && largest > 1536) {
        const frameRatio = Math.max(0.45, Math.min(1.0, idealFrameMax / maxFrameSide));
        targetRatio = Math.min(targetRatio, frameRatio);
      }
    }
  } else {
    const idealStaticMax = Math.max(960, displayMax * 1.5);
    if (largest > idealStaticMax * 1.2) {
      const staticRatio = Math.max(0.4, Math.min(1.0, idealStaticMax / largest));
      targetRatio = Math.min(targetRatio, staticRatio);
    }
  }

  if (targetRatio >= 0.92 && largest <= tierCap) {
    return 1.0;
  }
  return Math.max(0.25, Math.min(1.0, targetRatio));
}

export type NoteWindowState = 'pending' | 'hit' | 'missed';

export function advanceNoteSlidingWindow(
  notes: Array<{ timeMs: number; sustainMs: number }>,
  states: NoteWindowState[],
  activeStartIdx: number,
  songPosMs: number,
  lookbackMs = 240,
): { nextStartIdx: number; newlyDiscarded: number } {
  const len = Math.min(notes.length, states.length);
  let idx = activeStartIdx;
  let newlyDiscarded = 0;
  while (idx < len) {
    if (states[idx] === 'pending') break;
    const endMs = notes[idx].timeMs + notes[idx].sustainMs;
    if (endMs < songPosMs - lookbackMs) {
      idx += 1;
      newlyDiscarded += 1;
    } else {
      break;
    }
  }
  return { nextStartIdx: idx, newlyDiscarded };
}

export class LruResourceCache<T> {
  private entries = new Map<string, { value: T; bytes: number; tick: number }>();
  private currentBytes = 0;
  private tick = 0;

  constructor(
    private readonly maxBytes: number,
    private readonly maxEntries = 64,
  ) {}

  get(key: string): T | undefined {
    const slot = this.entries.get(key);
    if (!slot) return undefined;
    this.tick += 1;
    slot.tick = this.tick;
    return slot.value;
  }

  insert(key: string, value: T, sizeBytes: number): string[] {
    const prev = this.entries.get(key);
    if (prev) {
      this.currentBytes -= prev.bytes;
      this.entries.delete(key);
    }
    this.tick += 1;
    this.currentBytes += sizeBytes;
    this.entries.set(key, { value, bytes: sizeBytes, tick: this.tick });

    const evicted: string[] = [];
    while ((this.currentBytes > this.maxBytes || this.entries.size > this.maxEntries) && this.entries.size > 1) {
      let oldestKey: string | null = null;
      let oldestTick = Infinity;
      for (const [k, slot] of this.entries.entries()) {
        if (slot.tick < oldestTick) {
          oldestTick = slot.tick;
          oldestKey = k;
        }
      }
      if (!oldestKey) break;
      const removed = this.entries.get(oldestKey)!;
      this.entries.delete(oldestKey);
      this.currentBytes -= removed.bytes;
      evicted.push(oldestKey);
    }
    return evicted;
  }

  get bytesUsed(): number {
    return this.currentBytes;
  }

  has(key: string): boolean {
    return this.entries.has(key);
  }
}

export type MobileControlMode = 'tap_arrows' | 'classic_hitbox';

export function desaturateToNeutralRgba(rgba: Uint8ClampedArray | number[]): Uint8ClampedArray {
  const out = new Uint8ClampedArray(rgba.length);
  for (let i = 0; i + 3 < rgba.length; i += 4) {
    const a = rgba[i + 3];
    if (a > 0) {
      const luma = Math.min(212, Math.max(92, Math.round(0.299 * rgba[i] + 0.587 * rgba[i + 1] + 0.114 * rgba[i + 2])));
      out[i] = luma;
      out[i + 1] = luma;
      out[i + 2] = Math.min(255, luma + 6);
      out[i + 3] = Math.min(255, Math.round(a * 0.85));
    }
  }
  return out;
}

export interface SustainTailSpan {
  headVisible: boolean;
  tailTop: number;
  tailBottom: number;
  tailLength: number;
}

export function computeSustainTailSpan(params: {
  noteTimeMs: number;
  sustainMs: number;
  songPosMs: number;
  headHit: boolean;
  downscroll: boolean;
  receptorY: number;
  pxPerMs: number;
}): SustainTailSpan {
  const { noteTimeMs, sustainMs, songPosMs, headHit, downscroll, receptorY, pxPerMs } = params;
  const headDelta = noteTimeMs - songPosMs;
  const endDelta = noteTimeMs + sustainMs - songPosMs;
  const headY = downscroll ? receptorY - headDelta * pxPerMs : receptorY + headDelta * pxPerMs;
  const tailEndY = downscroll ? receptorY - endDelta * pxPerMs : receptorY + endDelta * pxPerMs;
  const anchorY = headHit ? receptorY : headY;
  const tailTop = downscroll ? tailEndY : anchorY;
  const tailBottom = downscroll ? anchorY : tailEndY;
  return {
    headVisible: !headHit,
    tailTop,
    tailBottom,
    tailLength: Math.max(0, tailBottom - tailTop),
  };
}

export interface ReceptorLayoutSpec {
  downscrollWideTactile: boolean;
  playerXs: [number, number, number, number];
  playerY: number;
  opponentXs: [number, number, number, number];
  opponentY: number;
}

export function computeReceptorLayout(params: {
  controlMode: MobileControlMode;
  downscroll: boolean;
  screenWidth: number;
  screenHeight: number;
  virtualWidth?: number;
  virtualHeight?: number;
}): ReceptorLayoutSpec {
  const {
    controlMode,
    downscroll,
    screenWidth,
    screenHeight,
    virtualWidth = 1280,
    virtualHeight = 720,
  } = params;
  const sx = Math.max(1, screenWidth) / Math.max(1, virtualWidth);
  const sy = Math.max(1, screenHeight) / Math.max(1, virtualHeight);
  const downscrollWideTactile = downscroll && controlMode === 'tap_arrows';

  if (downscrollWideTactile) {
    return {
      downscrollWideTactile: true,
      playerXs: [340 * sx, 495 * sx, 785 * sx, 940 * sx],
      playerY: 595 * sy,
      opponentXs: [72 * sx, 140 * sx, 208 * sx, 276 * sx],
      opponentY: 100 * sy,
    };
  }

  const y = (downscroll ? 575 : 118) * sy;
  return {
    downscrollWideTactile: false,
    playerXs: [765 * sx, 870 * sx, 975 * sx, 1080 * sx],
    playerY: y,
    opponentXs: [105 * sx, 210 * sx, 315 * sx, 420 * sx],
    opponentY: y,
  };
}

export function resolveMobileTouchLanes(params: {
  controlMode: MobileControlMode;
  touches: Array<{ x: number; y: number }>;
  screenWidth: number;
  screenHeight: number;
  downscroll: boolean;
  virtualWidth?: number;
  virtualHeight?: number;
}): number {
  const {
    controlMode,
    touches,
    screenWidth,
    screenHeight,
    downscroll,
    virtualWidth = 1280,
    virtualHeight = 720,
  } = params;
  const w = Math.max(1, screenWidth);
  const h = Math.max(1, screenHeight);
  let mask = 0;

  if (controlMode === 'classic_hitbox') {
    for (const t of touches) {
      const lane = Math.min(3, Math.max(0, Math.floor((t.x / w) * 4)));
      mask |= 1 << lane;
    }
    return mask;
  }

  const sx = w / Math.max(1, virtualWidth);
  const sy = h / Math.max(1, virtualHeight);
  const layout = computeReceptorLayout({
    controlMode,
    downscroll,
    screenWidth: w,
    screenHeight: h,
    virtualWidth,
    virtualHeight,
  });
  const notePxSize = (layout.downscrollWideTactile ? 98 : 84) * Math.min(sx, sy);
  const maxHitX = layout.downscrollWideTactile ? 82 * sx : Math.max(105 * sx * 0.54, notePxSize * 0.8);
  const maxHitY = Math.max(notePxSize * 1.3, 96);

  for (const t of touches) {
    if (Math.abs(t.y - layout.playerY) > maxHitY) continue;
    let bestLane = -1;
    let bestDist = Infinity;
    for (let dir = 0; dir < 4; dir++) {
      const cx = layout.playerXs[dir];
      const dx = Math.abs(t.x - cx);
      if (dx <= maxHitX && dx < bestDist) {
        bestDist = dx;
        bestLane = dir;
      }
    }
    if (bestLane >= 0) {
      mask |= 1 << bestLane;
    }
  }
  return mask;
}

export const BINARY_GAMEPLAY_MAGIC = 0x42464750; // "BFGP"
export const BINARY_GAMEPLAY_HEADER_BYTES = 152;
export const BINARY_GAMEPLAY_ENTITY_BYTES = 64;
export const BINARY_GAMEPLAY_NOTE_BYTES = 28;
export const BINARY_GAMEPLAY_STRUM_BYTES = 20;

export interface DecodedBinaryEntity {
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  rotationDeg: number;
  visible: boolean;
  flipX: boolean;
  hasFrame: boolean;
  frameRotated: boolean;
  roleCode: number;
  preloadRequested: boolean;
  retired: boolean;
  atlasX: number;
  atlasY: number;
  atlasW: number;
  atlasH: number;
  offX: number;
  offY: number;
  logicalW: number;
  logicalH: number;
}

export interface DecodedBinaryNote {
  id: bigint;
  timeMs: number;
  sustainMs: number;
  direction: number;
  isPlayer: boolean;
  headHit: boolean;
}

export interface DecodedStrumOverride {
  offsetX: number;
  offsetY: number;
  angle: number;
  scale: number;
  alpha: number;
}

export interface DecodedDynamicItem {
  isText: boolean;
  isHud: boolean;
  visible: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
  opacity: number;
  angle: number;
  colorArgb: number;
  text: string;
}

export interface DecodedBinaryGameplayFrame {
  active: boolean;
  downscroll: boolean;
  paused: boolean;
  finished: boolean;
  controlMode: MobileControlMode;
  hasHudStrings: boolean;
  hudVisible: boolean;
  flashOverHud: boolean;
  arrowsVisible: boolean;
  arrowsOpacity: number;
  screenBlackoutAlpha: number;
  hudRevision: number;
  holdingLanesMask: number;
  opponentHoldingLanesMask: number;
  lastRatingCode: number;
  splashLane: number;
  countdownStage: number;
  countdownProgress: number;
  songPosMs: number;
  durationMs: number;
  scrollSpeed: number;
  activeBpm: number;
  beatPhase: number;
  flashColorArgb: number;
  flashAlpha: number;
  activePlayerIdx: number;
  activeOpponentIdx: number;
  cameraZoom: number;
  cameraShake: number;
  cameraX: number;
  cameraY: number;
  virtualWidth: number;
  virtualHeight: number;
  health: number;
  hits: number;
  score: bigint;
  combo: number;
  maxCombo: number;
  misses: number;
  accuracyPct: number;
  hudOpacity: number;
  hudZoom: number;
  ratingEventId: number;
  splashEventId: number;
  entities: DecodedBinaryEntity[];
  visibleNotes: DecodedBinaryNote[];
  strumOverrides: DecodedStrumOverride[];
  dynamicItems: DecodedDynamicItem[];
  lastJudgement?: string;
  countdownLabel?: string;
  scriptError?: string;
  scriptLog?: string;
}

export function decodeBinaryGameplayFrame(
  input: ArrayBuffer | Uint8Array,
): DecodedBinaryGameplayFrame {
  const bytes =
    input instanceof Uint8Array
      ? input
      : new Uint8Array(input);
  if (bytes.byteLength < BINARY_GAMEPLAY_HEADER_BYTES) {
    throw new Error(`Paquete binario truncado (${bytes.byteLength} bytes)`);
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = view.getUint32(0, true);
  if (magic !== BINARY_GAMEPLAY_MAGIC) {
    throw new Error(`Cabecera mágica BFGP inválida: 0x${magic.toString(16)}`);
  }

  const flags = view.getUint32(4, true);
  const hudRevision = view.getUint32(8, true);
  const masks = view.getUint32(12, true);
  const countdownStage = view.getInt32(16, true);
  const countdownProgress = view.getFloat32(20, true);
  const songPosMs = view.getFloat64(24, true);
  const durationMs = view.getFloat64(32, true);
  const scrollSpeed = view.getFloat64(40, true);
  const activeBpm = view.getFloat64(48, true);
  const cameraZoom = view.getFloat32(56, true);
  const cameraShake = view.getFloat32(60, true);
  const cameraX = view.getFloat32(64, true);
  const cameraY = view.getFloat32(68, true);
  const virtualWidth = view.getFloat32(72, true);
  const virtualHeight = view.getFloat32(76, true);
  const health = view.getFloat32(80, true);
  const hits = view.getUint32(84, true);
  const score = view.getBigInt64(88, true);
  const combo = view.getUint32(96, true);
  const maxCombo = view.getUint32(100, true);
  const misses = view.getUint32(104, true);
  const accuracyPct = view.getFloat32(108, true);
  const hudOpacity = view.getFloat32(112, true);
  const hudZoom = view.getFloat32(116, true);
  const ratingEventId = view.getUint32(120, true);
  const splashEventId = view.getUint32(124, true);
  const beatPhase = view.getFloat32(128, true);
  const flashColorArgb = view.getUint32(132, true);
  const flashAlpha = view.getFloat32(136, true);
  const activePlayerIdx = view.getInt16(140, true);
  const activeOpponentIdx = view.getInt16(142, true);
  const entityCount = view.getUint16(144, true);
  const noteCount = view.getUint16(146, true);
  const strumCount = view.getUint16(148, true);
  const dynCount = view.getUint16(150, true);

  let cursor = BINARY_GAMEPLAY_HEADER_BYTES;
  const requiredMin =
    cursor +
    entityCount * BINARY_GAMEPLAY_ENTITY_BYTES +
    noteCount * BINARY_GAMEPLAY_NOTE_BYTES +
    strumCount * BINARY_GAMEPLAY_STRUM_BYTES;
  if (bytes.byteLength < requiredMin) {
    throw new Error(`Buffer insuficiente para entidades/notas/strums (${bytes.byteLength} < ${requiredMin})`);
  }

  const entities: DecodedBinaryEntity[] = [];
  for (let i = 0; i < entityCount; i++) {
    const x = view.getFloat32(cursor, true);
    const y = view.getFloat32(cursor + 4, true);
    const width = view.getFloat32(cursor + 8, true);
    const height = view.getFloat32(cursor + 12, true);
    const scale = view.getFloat32(cursor + 16, true);
    const opacity = view.getFloat32(cursor + 20, true);
    const rotationDeg = view.getFloat32(cursor + 24, true);
    const entFlags = view.getUint32(cursor + 28, true);
    const atlasX = view.getUint32(cursor + 32, true);
    const atlasY = view.getUint32(cursor + 36, true);
    const atlasW = view.getUint32(cursor + 40, true);
    const atlasH = view.getUint32(cursor + 44, true);
    const offX = view.getInt32(cursor + 48, true);
    const offY = view.getInt32(cursor + 52, true);
    const logicalW = view.getUint32(cursor + 56, true);
    const logicalH = view.getUint32(cursor + 60, true);
    cursor += BINARY_GAMEPLAY_ENTITY_BYTES;

    entities.push({
      x,
      y,
      width,
      height,
      scale,
      opacity,
      rotationDeg,
      visible: (entFlags & 1) !== 0,
      flipX: (entFlags & (1 << 1)) !== 0,
      hasFrame: (entFlags & (1 << 2)) !== 0,
      frameRotated: (entFlags & (1 << 3)) !== 0,
      roleCode: (entFlags >> 4) & 3,
      preloadRequested: (entFlags & (1 << 6)) !== 0,
      retired: (entFlags & (1 << 7)) !== 0,
      atlasX,
      atlasY,
      atlasW,
      atlasH,
      offX,
      offY,
      logicalW,
      logicalH,
    });
  }

  const visibleNotes: DecodedBinaryNote[] = [];
  for (let i = 0; i < noteCount; i++) {
    const id = view.getBigUint64(cursor, true);
    const timeMs = view.getFloat64(cursor + 8, true);
    const sustainMs = view.getFloat64(cursor + 16, true);
    const noteFlags = view.getUint32(cursor + 24, true);
    cursor += BINARY_GAMEPLAY_NOTE_BYTES;

    visibleNotes.push({
      id,
      timeMs,
      sustainMs,
      direction: noteFlags & 3,
      isPlayer: (noteFlags & (1 << 2)) !== 0,
      headHit: (noteFlags & (1 << 3)) !== 0,
    });
  }

  const strumOverrides: DecodedStrumOverride[] = [];
  for (let i = 0; i < strumCount; i++) {
    strumOverrides.push({
      offsetX: view.getFloat32(cursor, true),
      offsetY: view.getFloat32(cursor + 4, true),
      angle: view.getFloat32(cursor + 8, true),
      scale: view.getFloat32(cursor + 12, true),
      alpha: view.getFloat32(cursor + 16, true),
    });
    cursor += BINARY_GAMEPLAY_STRUM_BYTES;
  }

  const decoder = new TextDecoder('utf-8');
  const readStr = () => {
    if (cursor + 2 > bytes.byteLength) {
      throw new Error('String UTF-8 truncada en paquete BFGP');
    }
    const len = view.getUint16(cursor, true);
    cursor += 2;
    if (cursor + len > bytes.byteLength) {
      throw new Error('Longitud de string UTF-8 excede el buffer');
    }
    const text = decoder.decode(bytes.subarray(cursor, cursor + len));
    cursor += len;
    return text;
  };

  const dynamicItems: DecodedDynamicItem[] = [];
  for (let i = 0; i < dynCount; i++) {
    const isText = bytes[cursor] !== 0;
    const isHud = bytes[cursor + 1] !== 0;
    const visible = bytes[cursor + 2] !== 0;
    const x = view.getFloat32(cursor + 4, true);
    const y = view.getFloat32(cursor + 8, true);
    const width = view.getFloat32(cursor + 12, true);
    const height = view.getFloat32(cursor + 16, true);
    const opacity = view.getFloat32(cursor + 20, true);
    const angle = view.getFloat32(cursor + 24, true);
    const colorArgb = view.getUint32(cursor + 28, true);
    cursor += 32;
    const text = readStr();
    dynamicItems.push({ isText, isHud, visible, x, y, width, height, opacity, angle, colorArgb, text });
  }

  const hasHudStrings = (flags & (1 << 5)) !== 0;
  let lastJudgement: string | undefined;
  let countdownLabel: string | undefined;
  let scriptError: string | undefined;
  let scriptLog: string | undefined;

  if (hasHudStrings) {
    lastJudgement = readStr();
    countdownLabel = readStr();
    scriptError = readStr();
    scriptLog = readStr();
  }

  return {
    active: (flags & 1) !== 0,
    downscroll: (flags & (1 << 1)) !== 0,
    paused: (flags & (1 << 2)) !== 0,
    finished: (flags & (1 << 3)) !== 0,
    controlMode: (flags & (1 << 4)) !== 0 ? 'classic_hitbox' : 'tap_arrows',
    hasHudStrings,
    hudVisible: (flags & (1 << 6)) !== 0,
    flashOverHud: (flags & (1 << 7)) !== 0,
    arrowsVisible: (flags & (1 << 8)) === 0,
    arrowsOpacity: 1 - (((flags >>> 24) & 0xff) / 255),
    screenBlackoutAlpha: ((flags >>> 16) & 0xff) / 255,
    hudRevision,
    holdingLanesMask: masks & 0xff,
    opponentHoldingLanesMask: (masks >> 8) & 0xff,
    lastRatingCode: (masks >> 16) & 0xf,
    splashLane: (masks >> 20) & 0xf,
    countdownStage,
    countdownProgress,
    songPosMs,
    durationMs,
    scrollSpeed,
    activeBpm,
    beatPhase,
    flashColorArgb,
    flashAlpha,
    activePlayerIdx,
    activeOpponentIdx,
    cameraZoom,
    cameraShake,
    cameraX,
    cameraY,
    virtualWidth,
    virtualHeight,
    health,
    hits,
    score,
    combo,
    maxCombo,
    misses,
    accuracyPct,
    hudOpacity,
    hudZoom,
    ratingEventId,
    splashEventId,
    entities,
    visibleNotes,
    strumOverrides,
    dynamicItems,
    lastJudgement,
    countdownLabel,
    scriptError,
    scriptLog,
  };
}

export function clampEntityScale(scale: number): number {
  if (!Number.isFinite(scale)) return 1.0;
  return Math.min(6.0, Math.max(0.05, scale));
}

export interface ProjectSummaryItem {
  id: string;
  name: string;
  updatedAtMs: number;
  isActive: boolean;
}

export function sortProjectSummaries<T extends ProjectSummaryItem>(projects: T[]): T[] {
  return [...projects].sort((a, b) => {
    if (a.isActive !== b.isActive) return a.isActive ? -1 : 1;
    if (b.updatedAtMs !== a.updatedAtMs) return b.updatedAtMs - a.updatedAtMs;
    return a.name.localeCompare(b.name);
  });
}

export const STREAM_RING_FRAMES = 131_072;
export const STREAM_RING_BYTES = STREAM_RING_FRAMES * 4;

export interface AudioTrackRamSpec {
  frameCount: number;
  muted: boolean;
}

export interface MergedAudioRamEstimate {
  mergedTracks: number;
  sourceRamBytes: number;
  streamRingBytes: number;
  estimatedRamSavedKb: number;
}

/**
 * Estima la memoria PCM ahorrada al fusionar 1–3 pistas (almacenadas en estéreo i16 = 4 bytes/frame)
 * en un único WAV en disco leído por un buffer circular SPSC de 512 KB (`STREAM_RING_BYTES`).
 */
export function estimateMergedAudioRamSavings(
  tracks: AudioTrackRamSpec[],
): MergedAudioRamEstimate {
  let mergedTracks = 0;
  let sourceRamBytes = 0;

  for (const track of tracks.slice(0, 3)) {
    const frames = Math.max(0, Math.floor(track.frameCount));
    if (frames <= 0) continue;
    sourceRamBytes += frames * 4;
    if (!track.muted) {
      mergedTracks += 1;
    }
  }

  const streamRingBytes = mergedTracks > 0 ? STREAM_RING_BYTES : 0;
  const savedBytes = Math.max(0, sourceRamBytes - streamRingBytes);

  return {
    mergedTracks,
    sourceRamBytes,
    streamRingBytes,
    estimatedRamSavedKb: Math.floor(savedBytes / 1024),
  };
}

export type CameraFollowMode = 'beats' | 'sections';

export interface StageCameraConfigSpec {
  enabled: boolean;
  baseX: number;
  baseY: number;
  baseZoom: number;
  followMode: CameraFollowMode;
  followIntervalBeats: number;
  smoothSpeed: number;
  panStrength: number;
}

export interface CameraCaptureBox {
  centerX: number;
  centerY: number;
  width: number;
  height: number;
  zoom: number;
}

/**
 * Calcula el rectángulo lógico de captura de la cámara en el espacio virtual (1280×720).
 * Agrandar la caja de captura reduce `zoom` (aleja cámara); achicarla aumenta `zoom` (acerca cámara).
 */
export function computeCameraCaptureBox(
  baseX: number,
  baseY: number,
  baseZoom: number,
  virtualWidth = 1280,
  virtualHeight = 720,
): CameraCaptureBox {
  const safeZoom = Math.min(2.5, Math.max(0.35, Number.isFinite(baseZoom) ? baseZoom : 1.0));
  return {
    centerX: Math.min(2600, Math.max(-1000, Number.isFinite(baseX) ? baseX : 640)),
    centerY: Math.min(1800, Math.max(-1000, Number.isFinite(baseY) ? baseY : 360)),
    width: virtualWidth / safeZoom,
    height: virtualHeight / safeZoom,
    zoom: safeZoom,
  };
}

/**
 * Genera el bloque de configuración sincronizado dentro de `camera_system.lua`.
 */
export function renderCameraLuaConfigBlock(cfg: StageCameraConfigSpec): string {
  return [
    '-- [CAM_CFG_START]',
    `local CAM_ENABLED = ${cfg.enabled ? 'true' : 'false'}`,
    `local CAM_BASE_X = ${cfg.baseX.toFixed(1)}`,
    `local CAM_BASE_Y = ${cfg.baseY.toFixed(1)}`,
    `local CAM_BASE_ZOOM = ${cfg.baseZoom.toFixed(3)}`,
    `local CAM_MODE = "${cfg.followMode}"`,
    `local CAM_INTERVAL_BEATS = ${Math.max(1, Math.min(32, Math.round(cfg.followIntervalBeats)))}`,
    `local CAM_SMOOTH = ${cfg.smoothSpeed.toFixed(2)}`,
    `local CAM_PAN_STRENGTH = ${cfg.panStrength.toFixed(2)}`,
    '-- [CAM_CFG_END]',
  ].join('\n');
}

/**
 * Extrae parámetros editados manualmente desde `camera_system.lua` manteniendo valores válidos.
 */
export function parseCameraConfigFromLua(
  source: string,
  current: StageCameraConfigSpec,
): StageCameraConfigSpec {
  const next: StageCameraConfigSpec = { ...current };
  for (const line of source.split(/\r?\n/)) {
    const noComment = line.trim().split('--')[0]?.trim() ?? '';
    const eqIdx = noComment.indexOf('=');
    if (eqIdx <= 0) continue;
    const key = noComment.slice(0, eqIdx).trim().replace(/^local\s+/, '').trim();
    const val = noComment.slice(eqIdx + 1).trim();

    if (key === 'CAM_ENABLED') {
      if (/^true$/i.test(val)) next.enabled = true;
      else if (/^false$/i.test(val)) next.enabled = false;
    } else if (key === 'CAM_BASE_X') {
      const n = Number(val);
      if (Number.isFinite(n)) next.baseX = Math.min(2600, Math.max(-1000, n));
    } else if (key === 'CAM_BASE_Y') {
      const n = Number(val);
      if (Number.isFinite(n)) next.baseY = Math.min(1800, Math.max(-1000, n));
    } else if (key === 'CAM_BASE_ZOOM') {
      const n = Number(val);
      if (Number.isFinite(n)) next.baseZoom = Math.min(2.5, Math.max(0.35, n));
    } else if (key === 'CAM_MODE') {
      const clean = val.replace(/^['"]|['"]$/g, '').toLowerCase();
      next.followMode = clean.includes('section') || clean === 'chart' || clean === '1' ? 'sections' : 'beats';
    } else if (key === 'CAM_INTERVAL_BEATS') {
      const n = Number(val);
      if (Number.isFinite(n)) next.followIntervalBeats = Math.min(32, Math.max(1, Math.round(n)));
    } else if (key === 'CAM_SMOOTH') {
      const n = Number(val);
      if (Number.isFinite(n)) next.smoothSpeed = Math.min(20, Math.max(1, n));
    } else if (key === 'CAM_PAN_STRENGTH') {
      const n = Number(val);
      if (Number.isFinite(n)) next.panStrength = Math.min(1, Math.max(0, n));
    }
  }
  return next;
}

export interface BpmSyncedIdleCadence {
  beatSec: number;
  beatInterval: 1 | 2;
  shouldTriggerBeat: boolean;
  singReturnThresholdSec: number;
}

/**
 * Calcula la cadencia del Idle sincronizado al BPM activo de `chart.json`
 * (igual que `gameplay_ops.rs`: cada 2 beats si el ciclo Idle supera 1.35 beats, si no cada beat).
 */
export function computeBpmSyncedIdleCadence(params: {
  bpm: number;
  idleDurationSec: number;
  beatIndex: number;
}): BpmSyncedIdleCadence {
  const safeBpm = Math.max(20, Math.min(400, Number.isFinite(params.bpm) ? params.bpm : 120));
  const beatSec = 60 / safeBpm;
  const beatInterval: 1 | 2 = params.idleDurationSec > beatSec * 1.35 ? 2 : 1;
  const shouldTriggerBeat = ((params.beatIndex % beatInterval) + beatInterval) % beatInterval === 0;
  const singReturnThresholdSec = Math.min(0.58, Math.max(0.28, beatSec * 0.9));
  return {
    beatSec,
    beatInterval,
    shouldTriggerBeat,
    singReturnThresholdSec,
  };
}

export type HitRatingTier = 'sick' | 'good' | 'bad' | 'miss';

export interface HitJudgementResult {
  tier: HitRatingTier;
  ratingCode: 1 | 2 | 3 | 4;
  weight: number;
  scoreDelta: number;
  triggersSplash: boolean;
}

/**
 * Clasifica un golpe del jugador por ventana temporal (Sick <= 55ms, Good <= 105ms, Bad <= 155ms)
 * y determina si detona chispas de Note Splash (solo en `sick`).
 */
export function evaluateHitJudgement(absDiffMs: number): HitJudgementResult {
  if (absDiffMs <= 55) {
    return { tier: 'sick', ratingCode: 1, weight: 1.0, scoreDelta: 350, triggersSplash: true };
  }
  if (absDiffMs <= 105) {
    return { tier: 'good', ratingCode: 2, weight: 0.75, scoreDelta: 200, triggersSplash: false };
  }
  if (absDiffMs <= 155) {
    return { tier: 'bad', ratingCode: 3, weight: 0.4, scoreDelta: 100, triggersSplash: false };
  }
  return { tier: 'miss', ratingCode: 4, weight: 0.0, scoreDelta: -50, triggersSplash: false };
}

export interface FnfHealthBarLayout {
  dividerRatio: number; // 0.0 = 100% Verde (Player gana), 1.0 = 100% Rojo (Rival gana)
  playerLosing: boolean;
  opponentLosing: boolean;
}

/**
 * Calcula la proporción roja/verde de la barra FNF y cuándo activar íconos de peligro (<20%).
 */
export function evaluateFnfHealthBar(health: number): FnfHealthBarLayout {
  const clamped = Math.min(1, Math.max(0, Number.isFinite(health) ? health : 0.5));
  return {
    dividerRatio: 1 - clamped,
    playerLosing: clamped < 0.2,
    opponentLosing: clamped > 0.8,
  };
}

export interface FileFingerprintSpec {
  fileLenBytes: number;
  modifiedEpochMs: number;
  contentHash: string;
}

export interface TextureCacheFingerprintSpec {
  entityId: string;
  tier: 'low_32bit' | 'balanced' | 'high_quality';
  file: FileFingerprintSpec;
  hasAtlas: boolean;
  entityWidth: number;
  entityHeight: number;
  entityScale: number;
}

export interface AudioSlotCacheFingerprintSpec {
  slotIndex: number;
  file: FileFingerprintSpec;
  volume: number;
  muted: boolean;
}

export interface AudioMixCacheFingerprintSpec {
  sampleRate: number;
  slots: AudioSlotCacheFingerprintSpec[];
}

export interface ScriptCacheFingerprintSpec {
  scriptId: string;
  language: 'lua' | 'ts' | 'js';
  optimizedSourceHash: string;
  artifactExists: boolean;
}

export interface CompiledSpriteTargetSpec {
  mode: 'passthrough_webp' | 'encode_lossless_webp';
  outputPath: string;
  scaleFactor: number;
}

/**
 * Calcula una huella determinista FNV-1a de 64 bits en hexadecimal (16 caracteres),
 * equivalente a `fnv1a_64_bytes` / `fnv1a_64_str` en `pulse_core::hot_compile`.
 */
export function fnv1a64Hex(input: string | Uint8Array): string {
  const bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input;
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (let i = 0; i < bytes.length; i++) {
    hash ^= BigInt(bytes[i]);
    hash = (hash * prime) & mask;
  }
  return hash.toString(16).padStart(16, '0');
}

function sameFileFingerprint(a: FileFingerprintSpec, b: FileFingerprintSpec): boolean {
  return (
    a.fileLenBytes === b.fileLenBytes &&
    a.modifiedEpochMs === b.modifiedEpochMs &&
    a.contentHash === b.contentHash
  );
}

/**
 * Determina si una textura WebP compilada en caché puede reutilizarse sin re-decodificar ni re-codificar.
 */
export function shouldReuseCachedTexture(
  previous: TextureCacheFingerprintSpec | undefined,
  current: TextureCacheFingerprintSpec,
  cachedFileExists: boolean,
): boolean {
  if (!previous || !cachedFileExists) return false;
  return (
    previous.entityId === current.entityId &&
    previous.tier === current.tier &&
    previous.hasAtlas === current.hasAtlas &&
    Math.round(previous.entityWidth) === Math.round(current.entityWidth) &&
    Math.round(previous.entityHeight) === Math.round(current.entityHeight) &&
    Math.abs(previous.entityScale - current.entityScale) < 0.0005 &&
    sameFileFingerprint(previous.file, current.file)
  );
}

/**
 * Determina si `compiled_mix.wav` sigue vigente según pistas, volumen, silencio y sample rate.
 */
export function shouldReuseCachedAudio(
  previous: AudioMixCacheFingerprintSpec | undefined,
  current: AudioMixCacheFingerprintSpec,
  compiledWavExists: boolean,
): boolean {
  if (!previous || !compiledWavExists) return false;
  if (previous.sampleRate !== current.sampleRate) return false;
  if (previous.slots.length !== current.slots.length) return false;

  for (let i = 0; i < current.slots.length; i++) {
    const prev = previous.slots[i];
    const curr = current.slots[i];
    if (
      prev.slotIndex !== curr.slotIndex ||
      prev.muted !== curr.muted ||
      Math.abs(prev.volume - curr.volume) >= 0.001 ||
      !sameFileFingerprint(prev.file, curr.file)
    ) {
      return false;
    }
  }
  return true;
}

/**
 * Determina si el artefacto pre-compilado de script (`.luac` o `.js`) coincide con el hash optimizado.
 */
export function shouldReuseCachedScript(
  previous: ScriptCacheFingerprintSpec | undefined,
  current: Omit<ScriptCacheFingerprintSpec, 'artifactExists'>,
  artifactExists: boolean,
): boolean {
  if (!previous || !artifactExists) return false;
  return (
    previous.scriptId === current.scriptId &&
    previous.language === current.language &&
    previous.optimizedSourceHash === current.optimizedSourceHash
  );
}

/**
 * Decide si el sprite fuente ya es `.webp` en escala 1:1 (pass-through) o si debe compilarse
 * hacia `<cacheDir>/<entityId>.webp` mediante WebP lossless de máxima compresión.
 */
export function resolveCompiledSpriteTarget(params: {
  entityId: string;
  sourcePath: string;
  cacheDir: string;
  scaleFactor: number;
}): CompiledSpriteTargetSpec {
  const isAlreadyWebp = params.sourcePath.trim().toLowerCase().endsWith('.webp');
  const needsRescale = params.scaleFactor < 0.9999;
  if (isAlreadyWebp && !needsRescale) {
    return {
      mode: 'passthrough_webp',
      outputPath: params.sourcePath,
      scaleFactor: 1.0,
    };
  }
  const cleanDir = params.cacheDir.replace(/\/+$/, '');
  return {
    mode: 'encode_lossless_webp',
    outputPath: `${cleanDir}/${params.entityId}.webp`,
    scaleFactor: Math.min(1.0, Math.max(0.05, params.scaleFactor)),
  };
}

export interface WatchdogExecutionOutcome {
  timedOut: boolean;
  tripCount: number;
  quarantined: boolean;
  skippedDueToQuarantine: boolean;
}

/**
 * Modela el Watchdog real de scripts en runtime (`lua_vm.rs` / `js_vm.rs`):
 * presupuesto de 250 ms en `onCreate` o 25 ms por frame y cuarentena automática tras 3 disparos.
 */
export function evaluateWatchdogExecution(params: {
  phase: 'init' | 'frame';
  elapsedMs: number;
  previousTrips: number;
  maxTripsBeforeQuarantine?: number;
}): WatchdogExecutionOutcome {
  const maxTrips = params.maxTripsBeforeQuarantine ?? 3;
  if (params.previousTrips >= maxTrips) {
    return {
      timedOut: false,
      tripCount: params.previousTrips,
      quarantined: true,
      skippedDueToQuarantine: true,
    };
  }
  const budgetMs = params.phase === 'init' ? 250 : 25;
  const timedOut = params.elapsedMs > budgetMs;
  const tripCount = timedOut ? params.previousTrips + 1 : params.previousTrips;
  return {
    timedOut,
    tripCount,
    quarantined: tripCount >= maxTrips,
    skippedDueToQuarantine: false,
  };
}

export type ChartGridSustainKind = 'head' | 'body' | 'tail';

export interface ChartGridStepSustain {
  globalStep: number;
  sectionIndex: number;
  stepInSection: number;
  col: number;
  kind: ChartGridSustainKind;
}

/**
 * Calcula los segmentos continuos (head -> body -> tail) de una nota sostenida en la rejilla
 * del Chart Editor, propagándose incluso cuando cruza límites entre secciones.
 */
export function computeChartSustainGridCells(params: {
  sections: Array<{ bpm: number; lengthInSteps: number; startMs: number }>;
  notes: Array<{ timeMs: number; direction: number; isPlayer: boolean; sustainMs: number }>;
}): ChartGridStepSustain[] {
  const steps: Array<{ secIdx: number; stepInSec: number; timeMs: number; stepMs: number }> = [];
  params.sections.forEach((sec, secIdx) => {
    const stepMs = 60_000 / Math.max(40, sec.bpm) / 4;
    for (let s = 0; s < sec.lengthInSteps; s++) {
      steps.push({ secIdx, stepInSec: s, timeMs: sec.startMs + s * stepMs, stepMs });
    }
  });
  if (steps.length === 0) return [];

  const findClosest = (timeMs: number) => {
    let best = 0;
    let bestDiff = Math.abs(steps[0].timeMs - timeMs);
    for (let i = 1; i < steps.length; i++) {
      const diff = Math.abs(steps[i].timeMs - timeMs);
      if (diff < bestDiff) {
        bestDiff = diff;
        best = i;
      }
    }
    return best;
  };

  const out: ChartGridStepSustain[] = [];
  for (const n of params.notes) {
    const originIdx = findClosest(n.timeMs);
    const origin = steps[originIdx];
    if (n.sustainMs <= origin.stepMs * 0.4) continue;
    const endIdx = Math.min(steps.length - 1, Math.max(originIdx + 1, findClosest(n.timeMs + n.sustainMs)));
    const col = (n.isPlayer ? 4 : 0) + Math.max(0, Math.min(3, n.direction));

    out.push({
      globalStep: originIdx,
      sectionIndex: origin.secIdx,
      stepInSection: origin.stepInSec,
      col,
      kind: 'head',
    });
    for (let k = originIdx + 1; k <= endIdx; k++) {
      out.push({
        globalStep: k,
        sectionIndex: steps[k].secIdx,
        stepInSection: steps[k].stepInSec,
        col,
        kind: k === endIdx ? 'tail' : 'body',
      });
    }
  }
  return out;
}

export interface StageRamLifecycleReport {
  preservedIds: string[];
  reusedLaterIds: string[];
  preloadWindows: Array<{ entityId: string; preloadAtMs: number; cueAtMs: number }>;
}

/**
 * Analiza eventos cronológicos (`Stage Swap`, `Change Character`, `Preload Sprite`) para preservar
 * sprites ocultos al inicio en disco `.webp`, programar precarga anticipada 4s antes y detectar
 * sprites retirados que vuelven a aparecer más adelante.
 */
export function analyzeStageEventRamLifecycle(
  events: ChartEventSpec[],
  lookaheadMs = 4000,
): StageRamLifecycleReport {
  const sorted = [...events].sort((a, b) => a.timeMs - b.timeMs);
  const preservedSet = new Set<string>();
  const retiredAt = new Map<string, number>();
  const reusedSet = new Set<string>();
  const preloadWindows: Array<{ entityId: string; preloadAtMs: number; cueAtMs: number }> = [];

  for (const ev of sorted) {
    const lower = ev.name.trim().toLowerCase();
    if (lower !== 'stage swap' && lower !== 'swap stage' && lower !== 'change character' && lower !== 'preload sprite') {
      continue;
    }
    const kv = new Map<string, string>();
    for (const part of `${ev.value1};${ev.value2}`.split(';')) {
      const idx = part.indexOf('=');
      if (idx > 0) {
        kv.set(part.slice(0, idx).trim().toLowerCase(), part.slice(idx + 1).trim());
      }
    }

    const showIds: string[] = [];
    const hideIds: string[] = [];
    if (lower === 'preload sprite') {
      showIds.push(...ev.value1.split(',').map((s) => s.trim()).filter(Boolean));
    } else {
      for (const k of ['show', 'mostrar', 'player', 'jugador', 'opp', 'opponent', 'enemigo', 'rival']) {
        const val = kv.get(k);
        if (val) showIds.push(...val.split(',').map((s) => s.trim()).filter(Boolean));
      }
      for (const k of ['hide', 'ocultar']) {
        const val = kv.get(k);
        if (val) hideIds.push(...val.split(',').map((s) => s.trim()).filter(Boolean));
      }
    }

    for (const id of showIds) {
      preservedSet.add(id);
      preloadWindows.push({
        entityId: id,
        preloadAtMs: Math.max(0, ev.timeMs - lookaheadMs),
        cueAtMs: ev.timeMs,
      });
      const prevRetire = retiredAt.get(id);
      if (prevRetire !== undefined && ev.timeMs > prevRetire) {
        reusedSet.add(id);
      }
    }
    for (const id of hideIds) {
      preservedSet.add(id);
      retiredAt.set(id, ev.timeMs);
    }
  }

  return {
    preservedIds: Array.from(preservedSet),
    reusedLaterIds: Array.from(reusedSet),
    preloadWindows,
  };
}

export function buildStageSwapEventParams(params: {
  showIds?: string[];
  hideIds?: string[];
  playerId?: string;
  opponentId?: string;
  disposeOld?: boolean;
}): { value1: string; value2: string } {
  const parts: string[] = [];
  if (params.showIds && params.showIds.length > 0) {
    parts.push(`show=${params.showIds.join(',')}`);
  }
  if (params.hideIds && params.hideIds.length > 0) {
    parts.push(`hide=${params.hideIds.join(',')}`);
  }
  if (params.playerId) parts.push(`player=${params.playerId}`);
  if (params.opponentId) parts.push(`opp=${params.opponentId}`);
  parts.push(`dispose=${params.disposeOld ? 1 : 0}`);
  return { value1: parts.join(';'), value2: '' };
}

export function buildFlashEffectEventParams(params: {
  colorHex?: string;
  startMs: number;
  endMs: number;
  alpha?: number;
  overHud?: boolean;
}): { value1: string; value2: string } {
  const durSec = Math.max(0.04, (params.endMs - params.startMs) / 1000);
  const alpha = Math.min(1, Math.max(0.05, params.alpha ?? 0.85));
  return {
    value1: params.colorHex?.trim() || '#FFFFFF',
    value2: `dur=${durSec.toFixed(3)};endMs=${params.endMs.toFixed(2)};alpha=${alpha.toFixed(2)};hud=${params.overHud ? 1 : 0}`,
  };
}

export interface ChartPlayheadStepMatch {
  globalStep: number;
  sectionIndex: number;
  stepInSection: number;
  stepTimeMs: number;
  stepMs: number;
  stepFraction: number;
}

/**
 * Resuelve en qué paso `#` y fracción sub-paso (`0..1`) descansa el hilo reproductor
 * del Chart Editor respetando cambios de BPM entre secciones.
 */
export function resolveChartPlayheadStep(params: {
  sections: Array<{ bpm: number; lengthInSteps: number; startMs: number }>;
  songPosMs: number;
}): ChartPlayheadStepMatch | null {
  const steps: Array<{
    globalStep: number;
    sectionIndex: number;
    stepInSection: number;
    stepTimeMs: number;
    stepMs: number;
  }> = [];

  let globalStep = 0;
  params.sections.forEach((sec, sectionIndex) => {
    const stepMs = 60_000 / Math.max(40, sec.bpm) / 4;
    for (let s = 0; s < sec.lengthInSteps; s++) {
      steps.push({
        globalStep,
        sectionIndex,
        stepInSection: s,
        stepTimeMs: sec.startMs + s * stepMs,
        stepMs,
      });
      globalStep += 1;
    }
  });

  if (steps.length === 0) return null;
  const clampedMs = Math.max(0, params.songPosMs);
  let lo = 0;
  let hi = steps.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >>> 1;
    if (steps[mid].stepTimeMs <= clampedMs) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  const entry = steps[lo];
  const stepFraction = Math.min(1, Math.max(0, (clampedMs - entry.stepTimeMs) / Math.max(1, entry.stepMs)));
  return {
    ...entry,
    stepFraction,
  };
}

export function computeChartPlayheadFollowAction(params: {
  followThread: boolean;
  threadVisible: boolean;
  threadY: number;
  viewportStart: number;
  viewportEnd: number;
}): { action: 'none' | 'scroll_by' | 'snap_to_row'; deltaY: number } {
  if (!params.followThread) {
    return { action: 'none', deltaY: 0 };
  }
  if (!params.threadVisible || params.threadY < params.viewportStart) {
    return { action: 'snap_to_row', deltaY: 0 };
  }
  const viewportH = Math.max(1, params.viewportEnd - params.viewportStart);
  const anchorY = viewportH * 0.4;
  if (params.threadY > anchorY) {
    return { action: 'scroll_by', deltaY: params.threadY - anchorY };
  }
  return { action: 'none', deltaY: 0 };
}

export type SafImportKindKey =
  | 'chart'
  | 'audio_track'
  | 'sprite_png'
  | 'sparrow_xml'
  | 'script'
  | 'arrow_svg'
  | 'character_icon'
  | 'health_bar_svg'
  | 'mod_pkg';

export type SafStoragePolicy = 'staging_then_embed' | 'staging_then_adopt';

/**
 * Clasifica qué importaciones SAF viven únicamente en `cacheDir/import_staging/` antes de
 * incrustarse en `project.json` vs qué assets binarios se adoptan en `filesDir/projects/<id>/assets/`.
 */
export function classifySafImportStorage(kind: SafImportKindKey): {
  stagingDir: 'cacheDir/import_staging';
  policy: SafStoragePolicy;
  persistsBinaryInFilesDir: boolean;
  deleteStagedAfterImport: boolean;
} {
  const isPermanentBinary =
    kind === 'audio_track' ||
    kind === 'sprite_png' ||
    kind === 'character_icon' ||
    kind === 'mod_pkg';
  return {
    stagingDir: 'cacheDir/import_staging',
    policy: isPermanentBinary ? 'staging_then_adopt' : 'staging_then_embed',
    persistsBinaryInFilesDir: isPermanentBinary,
    deleteStagedAfterImport: true,
  };
}

export interface DiskCacheBudgetSplit {
  totalBytes: number;
  compiledCacheBytes: number;
  audioCacheBytes: number;
}

/**
 * Calcula la cuota de caché acotada entre 48 MiB y 384 MiB (predeterminado 160 MiB)
 * repartida 65% a caché multi-proyecto compilado y 35% a caché de audio decodificado `.bfaud`.
 */
export function computeDiskCacheBudgetSplit(reportedQuotaBytes: number): DiskCacheBudgetSplit {
  const MIN = 48 * 1024 * 1024;
  const MAX = 384 * 1024 * 1024;
  const DEFAULT = 160 * 1024 * 1024;
  const base = !Number.isFinite(reportedQuotaBytes) || reportedQuotaBytes <= 0 ? DEFAULT : reportedQuotaBytes;
  const totalBytes = Math.min(MAX, Math.max(MIN, Math.floor(base)));
  const compiledCacheBytes = Math.max(32 * 1024 * 1024, Math.floor((totalBytes * 65) / 100));
  const audioCacheBytes = Math.max(16 * 1024 * 1024, totalBytes - compiledCacheBytes);
  return { totalBytes, compiledCacheBytes, audioCacheBytes };
}

export interface DiskCacheProjectEntry {
  projectId: string;
  sizeBytes: number;
  lastUsedMs: number;
}

/**
 * Planifica qué directorios `compiled_cache/<projectId>/` expulsar en orden LRU
 * para respetar la cuota automática sin borrar nunca el proyecto activo.
 */
export function planMultiProjectLruEviction(
  entries: DiskCacheProjectEntry[],
  maxBytes: number,
  activeProjectId: string,
): { evictedIds: string[]; keptIds: string[]; bytesAfter: number } {
  let total = entries.reduce((acc, e) => acc + Math.max(0, e.sizeBytes), 0);
  if (total <= maxBytes) {
    return {
      evictedIds: [],
      keptIds: entries.map((e) => e.projectId),
      bytesAfter: total,
    };
  }

  const ordered = [...entries].sort(
    (a, b) => a.lastUsedMs - b.lastUsedMs || a.projectId.localeCompare(b.projectId),
  );
  const evictedSet = new Set<string>();
  for (const item of ordered) {
    if (total <= maxBytes) break;
    if (item.projectId === activeProjectId) continue;
    evictedSet.add(item.projectId);
    total -= Math.max(0, item.sizeBytes);
  }

  return {
    evictedIds: Array.from(evictedSet),
    keptIds: entries.filter((e) => !evictedSet.has(e.projectId)).map((e) => e.projectId),
    bytesAfter: Math.max(0, total),
  };
}

/**
 * Genera la clave determinista para caché binaria `.bfaud` en `cacheDir/audio_cache/`.
 */
export function buildBfaudCacheKey(params: {
  sizeBytes: number;
  modifiedMs: number;
  sampleHash: string;
  targetSampleRate: number;
}): string {
  const sig = `${params.sizeBytes}:${params.modifiedMs}:${params.sampleHash}:${params.targetSampleRate}`;
  return `trk_${fnv1a64Hex(sig)}_${params.targetSampleRate}.bfaud`;
}

export interface BfaudHeaderInfo {
  version: number;
  sampleRate: number;
  peakCount: number;
  sampleCount: number;
  expectedFileBytes: number;
}

/**
 * Valida la cabecera de 24 bytes de un artefacto binario `.bfaud` (`pulse_audio::audio_cache`).
 */
export function validateBfaudHeader(bytes: Uint8Array): BfaudHeaderInfo {
  if (bytes.length < 24) {
    throw new Error('Archivo .bfaud truncado (< 24 bytes)');
  }
  if (bytes[0] !== 0x42 || bytes[1] !== 0x46 || bytes[2] !== 0x41 || bytes[3] !== 0x55) {
    throw new Error('Magic .bfaud inválido (esperado BFAU)');
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = view.getUint32(4, true);
  const sampleRate = view.getUint32(8, true);
  const peakCount = view.getUint32(12, true);
  const sampleCountBig = view.getBigUint64(16, true);
  const sampleCount = Number(sampleCountBig);

  if (version !== 1 || sampleRate < 8000 || peakCount > 4096 || sampleCountBig % 2n !== 0n) {
    throw new Error('Cabecera .bfaud fuera de rango');
  }
  const expectedFileBytes = 24 + peakCount * 4 + sampleCount * 2;
  return {
    version,
    sampleRate,
    peakCount,
    sampleCount,
    expectedFileBytes,
  };
}

export const VANILLA_MAGIC_BYTES = [0x56, 0x4e, 0x4c, 0x41] as const; // "VNLA"
export const VANILLA_HEADER_BYTES = 24;
export const VANILLA_FORMAT_VERSION = 1;
export const VANILLA_FILE_EXTENSION = 'mod';
export const VANILLA_FLAG_OPTIMIZED_WEBP = 1 << 0;
export const VANILLA_FLAG_ASSET_DEDUP = 1 << 1;
export const MAX_VANILLA_MANIFEST_BYTES = 16 * 1024 * 1024;

export type VanillaEntryCodec = 'raw' | 'zstd';

export interface VanillaHeaderInfo {
  version: number;
  flags: number;
  optimizedWebp: boolean;
  assetDedup: boolean;
  manifestCompressedBytes: number;
  manifestUncompressedBytes: number;
  manifestFnv1a64Hex: string;
  dataSectionOffset: number;
}

/**
 * Espejo exacto de `pulse_core::vanilla::classify_vanilla_entry_codec`:
 * - JSON / XML / SVG / scripts Lua, TS, JS: Zstd nivel 12
 * - Audio WAV y texturas ASTC / BMP: Zstd nivel 9
 * - Audio/imagen ya comprimidos (OGG, MP3, WebP, JPG, PNG): Raw sin gasto extra
 */
export function classifyVanillaEntryCodec(relPath: string): {
  codec: VanillaEntryCodec;
  level: number;
} {
  const dot = relPath.lastIndexOf('.');
  const ext = dot >= 0 ? relPath.slice(dot + 1).toLowerCase() : '';
  switch (ext) {
    case 'json':
    case 'xml':
    case 'svg':
    case 'lua':
    case 'ts':
    case 'js':
    case 'txt':
    case 'md':
      return { codec: 'zstd', level: 12 };
    case 'wav':
    case 'astc':
    case 'bmp':
    case 'tga':
    case 'raw':
    case 'pcm':
      return { codec: 'zstd', level: 9 };
    case 'ogg':
    case 'oga':
    case 'mp3':
    case 'flac':
    case 'm4a':
    case 'aac':
    case 'webp':
    case 'jpg':
    case 'jpeg':
    case 'png':
      return { codec: 'raw', level: 0 };
    default:
      return { codec: 'zstd', level: 6 };
  }
}

/**
 * Valida estrictamente una ruta relativa dentro de un contenedor Vanilla `.mod` contra path traversal.
 */
export function validateVanillaRelPath(relPath: string): string {
  if (!relPath || relPath.trim().length === 0) {
    throw new Error('Ruta vacía dentro del contenedor Vanilla');
  }
  if (
    relPath.includes('\0') ||
    relPath.includes('\\') ||
    relPath.includes(':') ||
    relPath.startsWith('/')
  ) {
    throw new Error(`Ruta insegura en Vanilla: '${relPath}'`);
  }
  const segments = relPath.split('/');
  if (segments.length === 0) {
    throw new Error('Ruta sin componentes válidos en Vanilla');
  }
  for (const seg of segments) {
    if (!seg || seg === '.' || seg === '..') {
      throw new Error(`Segmento inválido en ruta Vanilla: '${relPath}'`);
    }
  }
  return relPath;
}

/**
 * Valida la cabecera binaria de 24 bytes del contenedor portátil Vanilla (`.mod` con Zstd).
 */
export function validateVanillaHeader(bytes: Uint8Array): VanillaHeaderInfo {
  if (bytes.length < VANILLA_HEADER_BYTES) {
    throw new Error('Archivo .mod Vanilla inválido o truncado (< 24 bytes)');
  }
  if (
    bytes[0] !== VANILLA_MAGIC_BYTES[0] ||
    bytes[1] !== VANILLA_MAGIC_BYTES[1] ||
    bytes[2] !== VANILLA_MAGIC_BYTES[2] ||
    bytes[3] !== VANILLA_MAGIC_BYTES[3]
  ) {
    throw new Error('Formato no compatible: falta firma mágica Vanilla (VNLA)');
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = view.getUint16(4, true);
  if (version !== VANILLA_FORMAT_VERSION) {
    throw new Error(`Versión de contenedor Vanilla no soportada: v${version}`);
  }
  const flags = view.getUint16(6, true);
  const manifestCompressedBytes = view.getUint32(8, true);
  const manifestUncompressedBytes = view.getUint32(12, true);
  const manifestFnv1a64 = view.getBigUint64(16, true);

  if (
    manifestCompressedBytes === 0 ||
    manifestCompressedBytes > MAX_VANILLA_MANIFEST_BYTES ||
    manifestUncompressedBytes === 0 ||
    manifestUncompressedBytes > MAX_VANILLA_MANIFEST_BYTES
  ) {
    throw new Error('Tamaño de manifiesto Vanilla fuera de límites seguros');
  }

  return {
    version,
    flags,
    optimizedWebp: (flags & VANILLA_FLAG_OPTIMIZED_WEBP) !== 0,
    assetDedup: (flags & VANILLA_FLAG_ASSET_DEDUP) !== 0,
    manifestCompressedBytes,
    manifestUncompressedBytes,
    manifestFnv1a64Hex: manifestFnv1a64.toString(16).padStart(16, '0'),
    dataSectionOffset: VANILLA_HEADER_BYTES + manifestCompressedBytes,
  };
}

export type CharacterAnimKind =
  | 'idle'
  | 'left'
  | 'down'
  | 'up'
  | 'right'
  | 'missLeft'
  | 'missDown'
  | 'missUp'
  | 'missRight';

export interface ModCharacterAnimDescriptor {
  animKey: CharacterAnimKind | null;
  animNameRaw: string;
  prefix: string;
  fps: number;
  looped: boolean;
  offset: [number, number];
  flipX: boolean | null;
}

export interface ModCharacterDescriptor {
  kind: 'character';
  formatLabel: string;
  position: [number, number] | null;
  isRelativePosition: boolean;
  cameraPosition: [number, number] | null;
  scale: number | null;
  flipX: boolean | null;
  animations: ModCharacterAnimDescriptor[];
}

export interface ModStagePropDescriptor {
  name: string;
  assetHint: string;
  position: [number, number];
  scale: number;
  scrollFactor: number;
  zIndex: number;
  flipX: boolean;
  alpha: number;
}

export interface ModStageDescriptor {
  kind: 'stage';
  formatLabel: string;
  defaultZoom: number | null;
  cameraStart: [number, number] | null;
  playerPos: [number, number] | null;
  opponentPos: [number, number] | null;
  gfPos: [number, number] | null;
  isBottomCenterCoords: boolean;
  hideGf: boolean;
  playerCamOffset: [number, number] | null;
  opponentCamOffset: [number, number] | null;
  gfCamOffset: [number, number] | null;
  playerZ: number | null;
  opponentZ: number | null;
  gfZ: number | null;
  props: ModStagePropDescriptor[];
}

export type DetectedModDescriptor = ModCharacterDescriptor | ModStageDescriptor;

/**
 * Mapea nombres estándar de animaciones (`idle`, `singLEFT`, `singUPmiss`, etc.)
 * hacia las claves de animación de BeatForge Studio.
 */
export function mapAnimNameToKind(raw: string): CharacterAnimKind | null {
  const clean = raw.trim().toLowerCase().replace(/[\s\-_]/g, '');
  if (!clean) return null;

  if (clean.includes('miss') || clean.includes('fallo')) {
    if (clean.includes('left') || clean.includes('izq')) return 'missLeft';
    if (clean.includes('down') || clean.includes('abajo')) return 'missDown';
    if (clean.includes('up') || clean.includes('arriba')) return 'missUp';
    if (clean.includes('right') || clean.includes('der')) return 'missRight';
    return null;
  }

  if (
    clean === 'idle' ||
    clean === 'dance' ||
    clean === 'danceleft' ||
    clean === 'danceright' ||
    clean === 'reposo' ||
    clean.startsWith('idlealt')
  ) {
    return 'idle';
  }
  if (clean === 'singleft' || clean === 'left' || clean === 'izquierda' || clean.startsWith('singleft')) {
    return 'left';
  }
  if (clean === 'singdown' || clean === 'down' || clean === 'abajo' || clean.startsWith('singdown')) {
    return 'down';
  }
  if (clean === 'singup' || clean === 'up' || clean === 'arriba' || clean.startsWith('singup')) {
    return 'up';
  }
  if (clean === 'singright' || clean === 'right' || clean === 'derecha' || clean.startsWith('singright')) {
    return 'right';
  }
  return null;
}

/**
 * Devuelve la posición base predeterminada de BeatForge (`1280x720`) para cada rol.
 */
export function defaultRoleAnchor(role: EntityRole): [number, number] {
  switch (role) {
    case 'background':
      return [640, 360];
    case 'decoration':
      return [640, 430];
    case 'opponent':
      return [300, 420];
    case 'player':
      return [980, 430];
  }
}

function clampFinite(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, value));
}

/**
 * Convierte coordenadas de un `stage.json` / `stage.xml` hacia el lienzo virtual `1280x720`.
 */
export function normalizeStageWorldPos(
  rawPos: [number, number],
  role: EntityRole,
  isBottomCenterCoords: boolean,
): [number, number] {
  const [rx, ry] = rawPos;
  if (isBottomCenterCoords) {
    return [clampFinite(rx, -400, 1700), clampFinite(ry * 0.56, 120, 680)];
  }
  const halfW = role === 'background' ? 640 : role === 'decoration' ? 120 : 140;
  const halfH = role === 'background' ? 360 : role === 'decoration' ? 110 : 180;
  return [
    clampFinite(rx + halfW, -600, 2000),
    clampFinite(ry + halfH, -400, 1200),
  ];
}

/**
 * Calcula la posición final de una entidad al aplicar `position` de un descriptor de personaje.
 */
export function applyCharacterPosition(
  currentPos: [number, number],
  charPos: [number, number],
  isRelative: boolean,
  role: EntityRole,
): [number, number] {
  const [px, py] = charPos;
  if (!Number.isFinite(px) || !Number.isFinite(py)) return currentPos;
  if (!isRelative || (Math.abs(px) > 460 && py > 40)) {
    return [clampFinite(px, -1000, 2600), clampFinite(py, -1000, 1800)];
  }
  const anchor = defaultRoleAnchor(role);
  return [
    clampFinite(anchor[0] + px * 0.55, -1000, 2600),
    clampFinite(anchor[1] + py * 0.25, -1000, 1800),
  ];
}

function parseDescriptorVec2(val: unknown): [number, number] | null {
  if (!Array.isArray(val) || val.length < 2) return null;
  const x = asFiniteNumber(val[0]);
  const y = asFiniteNumber(val[1]);
  if (x === null || y === null) return null;
  return [x, y];
}

function parseXmlTagAttributes(attrChunk: string): Record<string, string> {
  const attrs: Record<string, string> = {};
  const attrRegex = /([A-Za-z_:][\w:.-]*)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  let match: RegExpExecArray | null;
  while ((match = attrRegex.exec(attrChunk)) !== null) {
    attrs[match[1]] = (match[2] ?? match[3] ?? '').trim();
  }
  return attrs;
}

function parseXmlBool(raw: string | undefined): boolean | null {
  if (!raw) return null;
  const low = raw.trim().toLowerCase();
  if (low === 'true' || low === '1' || low === 'yes') return true;
  if (low === 'false' || low === '0' || low === 'no') return false;
  return null;
}

function parseCharacterJsonDescriptor(obj: Record<string, unknown>): ModCharacterDescriptor {
  const animsRaw = Array.isArray(obj.animations) ? obj.animations : [];
  const firstAnim =
    animsRaw.length > 0 && animsRaw[0] && typeof animsRaw[0] === 'object'
      ? (animsRaw[0] as Record<string, unknown>)
      : null;
  const isVslice =
    'assetPath' in obj ||
    'startingAnimation' in obj ||
    (firstAnim !== null && ('prefix' in firstAnim || 'looped' in firstAnim));

  const position = parseDescriptorVec2(obj.position) ?? parseDescriptorVec2(obj.offsets);
  const cameraPosition = parseDescriptorVec2(obj.camera_position);
  const rawScale = asFiniteNumber(obj.scale);
  const scale = rawScale !== null ? clampFinite(rawScale, 0.05, 6.0) : null;
  const flipX =
    typeof obj.flip_x === 'boolean'
      ? obj.flip_x
      : typeof obj.flipX === 'boolean'
        ? obj.flipX
        : null;

  const animations: ModCharacterAnimDescriptor[] = [];
  for (const item of animsRaw) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) continue;
    const aobj = item as Record<string, unknown>;
    const animNameRaw =
      typeof (aobj.anim ?? aobj.name) === 'string' ? String(aobj.anim ?? aobj.name).trim() : '';
    const prefix =
      typeof aobj.prefix === 'string'
        ? aobj.prefix.trim()
        : 'anim' in aobj && typeof aobj.name === 'string'
          ? aobj.name.trim()
          : animNameRaw;
    if (!animNameRaw && !prefix) continue;

    const fps = clampFinite(asFiniteNumber(aobj.fps) ?? 24, 1, 120);
    const looped =
      typeof aobj.loop === 'boolean'
        ? aobj.loop
        : typeof aobj.looped === 'boolean'
          ? aobj.looped
          : animNameRaw.toLowerCase().includes('idle');
    const rawOff = parseDescriptorVec2(aobj.offsets) ?? [0, 0];
    const offset: [number, number] = isVslice ? rawOff : [-rawOff[0], -rawOff[1]];
    const animFlip = typeof aobj.flipX === 'boolean' ? aobj.flipX : null;

    animations.push({
      animKey: mapAnimNameToKind(animNameRaw),
      animNameRaw,
      prefix,
      fps,
      looped,
      offset,
      flipX: animFlip,
    });
  }

  if (animations.length === 0 && position === null && scale === null && flipX === null) {
    throw new Error('El JSON de personaje no contiene animaciones ni posición');
  }

  return {
    kind: 'character',
    formatLabel: isVslice ? 'Personaje JSON (V-Slice)' : 'Personaje JSON (Psych)',
    position,
    isRelativePosition: !isVslice,
    cameraPosition,
    scale,
    flipX,
    animations,
  };
}

function parseStageJsonDescriptor(obj: Record<string, unknown>): ModStageDescriptor {
  const chars =
    obj.characters && typeof obj.characters === 'object' && !Array.isArray(obj.characters)
      ? (obj.characters as Record<string, unknown>)
      : null;
  const rawZoom = asFiniteNumber(obj.defaultZoom ?? obj.cameraZoom);
  const defaultZoom = rawZoom !== null ? clampFinite(rawZoom, 0.35, 2.5) : null;
  const hideGf = typeof obj.hide_girlfriend === 'boolean' ? obj.hide_girlfriend : false;

  let playerPos = parseDescriptorVec2(obj.boyfriend);
  let opponentPos = parseDescriptorVec2(obj.opponent);
  let gfPos = parseDescriptorVec2(obj.girlfriend);
  let playerCamOffset = parseDescriptorVec2(obj.camera_boyfriend);
  let opponentCamOffset = parseDescriptorVec2(obj.camera_opponent);
  let gfCamOffset = parseDescriptorVec2(obj.camera_girlfriend);
  let playerZ: number | null = null;
  let opponentZ: number | null = null;
  let gfZ: number | null = null;

  if (chars) {
    const bf = (chars.bf ?? chars.boyfriend) as Record<string, unknown> | undefined;
    if (bf && typeof bf === 'object') {
      playerPos = parseDescriptorVec2(bf.position) ?? playerPos;
      playerCamOffset = parseDescriptorVec2(bf.cameraOffsets) ?? playerCamOffset;
      const z = asFiniteNumber(bf.zIndex);
      if (z !== null) playerZ = Math.round(z);
    }
    const dad = (chars.dad ?? chars.opponent) as Record<string, unknown> | undefined;
    if (dad && typeof dad === 'object') {
      opponentPos = parseDescriptorVec2(dad.position) ?? opponentPos;
      opponentCamOffset = parseDescriptorVec2(dad.cameraOffsets) ?? opponentCamOffset;
      const z = asFiniteNumber(dad.zIndex);
      if (z !== null) opponentZ = Math.round(z);
    }
    const gf = (chars.gf ?? chars.girlfriend) as Record<string, unknown> | undefined;
    if (gf && typeof gf === 'object') {
      gfPos = parseDescriptorVec2(gf.position) ?? gfPos;
      gfCamOffset = parseDescriptorVec2(gf.cameraOffsets) ?? gfCamOffset;
      const z = asFiniteNumber(gf.zIndex);
      if (z !== null) gfZ = Math.round(z);
    }
  }

  const props: ModStagePropDescriptor[] = [];
  const rawProps = Array.isArray(obj.props) ? obj.props : [];
  rawProps.forEach((item, idx) => {
    if (!item || typeof item !== 'object' || Array.isArray(item)) return;
    const pobj = item as Record<string, unknown>;
    const name =
      typeof pobj.name === 'string' && pobj.name.trim().length > 0 ? pobj.name.trim() : 'prop';
    const assetHint = typeof pobj.assetPath === 'string' ? pobj.assetPath.trim() : '';
    const position = parseDescriptorVec2(pobj.position) ?? [640, 360];
    const scaleVec = parseDescriptorVec2(pobj.scale);
    const scale = scaleVec
      ? clampFinite((scaleVec[0] + scaleVec[1]) * 0.5, 0.05, 6.0)
      : clampFinite(asFiniteNumber(pobj.scale) ?? 1.0, 0.05, 6.0);
    const scrollVec = parseDescriptorVec2(pobj.scroll);
    const scrollFactor = scrollVec ? clampFinite((scrollVec[0] + scrollVec[1]) * 0.5, 0, 2.0) : 1.0;
    const zIndex = Math.round(asFiniteNumber(pobj.zIndex) ?? idx * 5);
    const flipX = typeof pobj.flipX === 'boolean' ? pobj.flipX : false;
    const alpha = clampFinite(asFiniteNumber(pobj.alpha) ?? 1.0, 0, 1.0);
    props.push({ name, assetHint, position, scale, scrollFactor, zIndex, flipX, alpha });
  });

  return {
    kind: 'stage',
    formatLabel: chars !== null ? 'Escenario JSON (V-Slice)' : 'Escenario JSON (Psych)',
    defaultZoom,
    cameraStart: null,
    playerPos,
    opponentPos,
    gfPos,
    isBottomCenterCoords: chars !== null,
    hideGf,
    playerCamOffset,
    opponentCamOffset,
    gfCamOffset,
    playerZ,
    opponentZ,
    gfZ,
    props,
  };
}

function parseCharacterXmlDescriptor(xml: string): ModCharacterDescriptor {
  const rootMatch = xml.match(/<character\b([^>]*)>([\s\S]*?)<\/character>|<character\b([^>]*)\/\s*>/i);
  if (!rootMatch) {
    throw new Error('XML <character> mal formado');
  }
  const attrs = parseXmlTagAttributes(rootMatch[1] ?? rootMatch[3] ?? '');
  const body = rootMatch[2] ?? '';
  const x = asFiniteNumber(attrs.x);
  const y = asFiniteNumber(attrs.y);
  const position: [number, number] | null =
    x !== null || y !== null ? [x ?? 0, y ?? 0] : null;
  const cx = asFiniteNumber(attrs.camx);
  const cy = asFiniteNumber(attrs.camy);
  const cameraPosition: [number, number] | null =
    cx !== null || cy !== null ? [cx ?? 0, cy ?? 0] : null;
  const rawScale = asFiniteNumber(attrs.scale);
  const scale = rawScale !== null ? clampFinite(rawScale, 0.05, 6.0) : null;
  const flipX = parseXmlBool(attrs.flipX ?? attrs.flip_x);

  const animations: ModCharacterAnimDescriptor[] = [];
  const animRegex = /<anim\b([^>]*)\/?\s*>/gi;
  let m: RegExpExecArray | null;
  while ((m = animRegex.exec(body)) !== null) {
    const a = parseXmlTagAttributes(m[1]);
    const animNameRaw = a.name ?? '';
    const prefix = a.anim ?? a.prefix ?? animNameRaw;
    if (!animNameRaw && !prefix) continue;
    const fps = clampFinite(asFiniteNumber(a.fps) ?? 24, 1, 120);
    const looped = parseXmlBool(a.loop) ?? animNameRaw.toLowerCase().includes('idle');
    const ox = asFiniteNumber(a.x) ?? 0;
    const oy = asFiniteNumber(a.y) ?? 0;
    animations.push({
      animKey: mapAnimNameToKind(animNameRaw),
      animNameRaw,
      prefix,
      fps,
      looped,
      offset: [-ox, -oy],
      flipX: parseXmlBool(a.flipX),
    });
  }

  if (animations.length === 0 && position === null && scale === null && flipX === null) {
    throw new Error('El XML <character> no contiene animaciones ni atributos de posición');
  }

  return {
    kind: 'character',
    formatLabel: 'Personaje XML (Codename)',
    position,
    isRelativePosition: true,
    cameraPosition,
    scale,
    flipX,
    animations,
  };
}

function parseStageXmlDescriptor(xml: string): ModStageDescriptor {
  const rootMatch = xml.match(/<stage\b([^>]*)>([\s\S]*?)<\/stage>|<stage\b([^>]*)\/\s*>/i);
  if (!rootMatch) {
    throw new Error('XML <stage> mal formado');
  }
  const attrs = parseXmlTagAttributes(rootMatch[1] ?? rootMatch[3] ?? '');
  const body = rootMatch[2] ?? '';
  const rawZoom = asFiniteNumber(attrs.zoom);
  const defaultZoom = rawZoom !== null ? clampFinite(rawZoom, 0.35, 2.5) : null;
  const startX = asFiniteNumber(attrs.startCamPosX);
  const startY = asFiniteNumber(attrs.startCamPosY);
  const cameraStart: [number, number] | null =
    startX !== null && startY !== null ? [startX, startY] : null;

  let playerPos: [number, number] | null = null;
  let opponentPos: [number, number] | null = null;
  let gfPos: [number, number] | null = null;
  let playerCamOffset: [number, number] | null = null;
  let opponentCamOffset: [number, number] | null = null;
  let gfCamOffset: [number, number] | null = null;
  const props: ModStagePropDescriptor[] = [];

  const elemRegex = /<([A-Za-z_][\w:-]*)\b([^>]*)\/?\s*>/g;
  let m: RegExpExecArray | null;
  let idx = 0;
  while ((m = elemRegex.exec(body)) !== null) {
    const tag = m[1].toLowerCase();
    const a = parseXmlTagAttributes(m[2]);
    const x = asFiniteNumber(a.x);
    const y = asFiniteNumber(a.y);
    const pos: [number, number] | null = x !== null || y !== null ? [x ?? 0, y ?? 0] : null;
    const cx = asFiniteNumber(a.camxoffset);
    const cy = asFiniteNumber(a.camyoffset);
    const camOff: [number, number] | null = cx !== null && cy !== null ? [cx, cy] : null;

    if (tag === 'boyfriend' || tag === 'bf' || tag === 'player') {
      playerPos = pos ?? playerPos;
      playerCamOffset = camOff ?? playerCamOffset;
    } else if (tag === 'dad' || tag === 'opponent') {
      opponentPos = pos ?? opponentPos;
      opponentCamOffset = camOff ?? opponentCamOffset;
    } else if (tag === 'girlfriend' || tag === 'gf') {
      gfPos = pos ?? gfPos;
      gfCamOffset = camOff ?? gfCamOffset;
    } else if (tag === 'sprite' || tag === 'solid' || tag === 'box') {
      const sx = asFiniteNumber(a.scrollx) ?? 1.0;
      const sy = asFiniteNumber(a.scrolly) ?? 1.0;
      props.push({
        name: a.name && a.name.length > 0 ? a.name : 'prop',
        assetHint: a.sprite ?? '',
        position: pos ?? [640, 360],
        scale: clampFinite(asFiniteNumber(a.scale) ?? 1.0, 0.05, 6.0),
        scrollFactor: clampFinite((sx + sy) * 0.5, 0, 2.0),
        zIndex: idx * 5,
        flipX: parseXmlBool(a.flipX) ?? false,
        alpha: clampFinite(asFiniteNumber(a.alpha) ?? 1.0, 0, 1.0),
      });
    }
    idx += 1;
  }

  return {
    kind: 'stage',
    formatLabel: 'Escenario XML (Codename)',
    defaultZoom,
    cameraStart,
    playerPos,
    opponentPos,
    gfPos,
    isBottomCenterCoords: false,
    hideGf: false,
    playerCamOffset,
    opponentCamOffset,
    gfCamOffset,
    playerZ: null,
    opponentZ: null,
    gfZ: null,
    props,
  };
}

/**
 * Autodetecta e interpreta un archivo `.json` o `.xml` de configuración de personaje o escenario
 * (`characters/*.json|xml`, `stages/*.json|xml`), espejo de `pulse_core::mod_descriptors`.
 */
export function parseModDescriptor(raw: string): DetectedModDescriptor {
  const trimmed = raw.replace(/^\uFEFF/, '').trim();
  if (!trimmed) {
    throw new Error('El archivo de configuración está vacío');
  }
  if (trimmed.startsWith('{')) {
    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('El JSON debe tener un objeto raíz');
    }
    const obj = parsed as Record<string, unknown>;
    const looksStage =
      'boyfriend' in obj ||
      'opponent' in obj ||
      'girlfriend' in obj ||
      'characters' in obj ||
      Array.isArray(obj.props) ||
      'defaultZoom' in obj ||
      'cameraZoom' in obj;
    if (looksStage) {
      return parseStageJsonDescriptor(obj);
    }
    const looksChar =
      Array.isArray(obj.animations) ||
      'camera_position' in obj ||
      'healthicon' in obj ||
      'sing_duration' in obj ||
      ('position' in obj && 'image' in obj) ||
      'startingAnimation' in obj;
    if (looksChar) {
      return parseCharacterJsonDescriptor(obj);
    }
    throw new Error(
      'El JSON no coincide con un descriptor de personaje (animations/position) ni de escenario (boyfriend/opponent/characters/props)',
    );
  }
  if (trimmed.startsWith('<')) {
    if (/<character\b/i.test(trimmed)) {
      return parseCharacterXmlDescriptor(trimmed);
    }
    if (/<stage\b/i.test(trimmed)) {
      return parseStageXmlDescriptor(trimmed);
    }
    throw new Error('Etiqueta raíz XML no es <character> ni <stage>');
  }
  throw new Error('Formato no reconocido (se esperaba JSON o XML de personaje/escenario)');
}

export type UiBlackoutAction = 'intro' | 'hide' | 'show' | 'toggle' | 'fade';

export interface UiBlackoutCueSpec {
  isUiBlackoutEvent: boolean;
  isIntroBlackout: boolean;
  action: UiBlackoutAction;
  blackAlpha: number;
  hudAlpha: number;
  arrowsAlpha: number;
  durationSec: number;
  revealDelayMs: number;
  revealOnSongStart: boolean;
  ease: string;
}

const UI_BLACKOUT_EVENT_NAMES = new Set([
  'ui blackout',
  'blackout',
  'screen fade',
  'camera fade',
  'hide ui',
  'show ui',
  'toggle ui',
  'fade ui',
  'hud fade',
  'hide hud',
  'show hud',
]);

/**
 * Indica si el nombre de evento de chart corresponde a control de pantalla negra / visibilidad UI
 * (incluyendo eventos comunitarios de FNF/Psych como "Screen Fade", "Camera Fade", "Hide UI" o "Toggle UI").
 */
export function isUiBlackoutEventName(name: string): boolean {
  return UI_BLACKOUT_EVENT_NAMES.has(name.trim().toLowerCase());
}

function parseOptionalBoolFlag(raw: string): boolean | null {
  switch (raw.trim().toLowerCase()) {
    case '1':
    case 'true':
    case 'si':
    case 'sí':
    case 'yes':
    case 'on':
      return true;
    case '0':
    case 'false':
    case 'no':
    case 'off':
      return false;
    default:
      return null;
  }
}

/**
 * Construye `name`, `value1` y `value2` para un evento `"UI Blackout"` compatible con
 * `ui_blackout.lua` y el diálogo `#` del Chart Editor.
 */
export function buildUiBlackoutEventParams(params: {
  action: UiBlackoutAction;
  blackAlpha?: number;
  hideHud?: boolean;
  hideArrows?: boolean;
  durationSec?: number;
  revealDelayMs?: number;
  revealOnSongStart?: boolean;
  ease?: string;
}): { name: string; value1: string; value2: string } {
  const action = params.action;
  const targetBlack =
    action === 'show'
      ? 0
      : Math.min(1, Math.max(0, Number.isFinite(params.blackAlpha) ? (params.blackAlpha as number) : 1));
  const targetHud = action === 'show' ? 1 : (params.hideHud ?? true) ? 0 : 1;
  const targetArrows = action === 'show' ? 1 : (params.hideArrows ?? true) ? 0 : 1;
  const dur = Math.min(25, Math.max(0, Number.isFinite(params.durationSec) ? (params.durationSec as number) : 0.65));
  const revealMs =
    action === 'intro'
      ? Math.min(60000, Math.max(0, Number.isFinite(params.revealDelayMs) ? (params.revealDelayMs as number) : 0))
      : 0;
  const onStart = params.revealOnSongStart ?? true;
  const ease = params.ease?.trim();
  const parts = [
    `black=${targetBlack.toFixed(2)}`,
    `hud=${targetHud.toFixed(2)}`,
    `arrows=${targetArrows.toFixed(2)}`,
    `dur=${dur.toFixed(2)}`,
    `revealMs=${Math.round(revealMs)}`,
    `onStart=${onStart ? 1 : 0}`,
  ];
  if (ease) {
    parts.push(`ease=${ease}`);
  }
  return {
    name: 'UI Blackout',
    value1: action,
    value2: parts.join(';'),
  };
}

/**
 * Parsea un evento de chart hacia la especificación usada por `render_ui_blackout_lua_script`
 * en Rust (`pulse_core::project`), soportando tanto el formato nativo BeatForge como charts externos.
 */
export function parseUiBlackoutEvent(ev: {
  timeMs?: number;
  name: string;
  value1: string;
  value2: string;
}): UiBlackoutCueSpec {
  const lowerName = ev.name.trim().toLowerCase();
  const timeMs = Math.max(0, ev.timeMs ?? 0);
  if (!isUiBlackoutEventName(lowerName)) {
    return {
      isUiBlackoutEvent: false,
      isIntroBlackout: false,
      action: 'fade',
      blackAlpha: -1,
      hudAlpha: -1,
      arrowsAlpha: -1,
      durationSec: 0.6,
      revealDelayMs: 0,
      revealOnSongStart: true,
      ease: 'sineInOut',
    };
  }

  const v1Lower = ev.value1.trim().toLowerCase();
  let action: UiBlackoutAction =
    lowerName === 'hide ui' || lowerName === 'hide hud'
      ? 'hide'
      : lowerName === 'show ui' || lowerName === 'show hud'
        ? 'show'
        : lowerName === 'toggle ui'
          ? 'toggle'
          : v1Lower === 'intro' || v1Lower === 'hide' || v1Lower === 'show' || v1Lower === 'toggle' || v1Lower === 'fade'
            ? v1Lower
            : v1Lower === 'reveal'
              ? 'show'
              : 'fade';

  let blackAlpha =
    action === 'intro' || action === 'hide'
      ? lowerName === 'hide ui' || lowerName === 'hide hud'
        ? -1
        : 1
      : action === 'show'
        ? 0
        : -1;
  let hudAlpha = action === 'intro' || action === 'hide' ? 0 : action === 'show' ? 1 : -1;
  let arrowsAlpha = action === 'intro' || action === 'hide' ? 0 : action === 'show' ? 1 : -1;
  let durationSec = action === 'hide' && timeMs <= 50 ? 0 : 0.6;
  let revealDelayMs = 0;
  let revealOnSongStart = true;
  let ease = 'sineInOut';

  if (lowerName === 'screen fade' || lowerName === 'camera fade') {
    const parts = ev.value1.split(',').map((s) => s.trim());
    const candidate = parts.length === 1 ? Number(parts[0]) : Number(parts[parts.length - 1]);
    if (Number.isFinite(candidate) && candidate >= 0 && candidate <= 1) {
      blackAlpha = candidate;
    }
    if (blackAlpha < 0) blackAlpha = 1;
    const d = Number(ev.value2.trim());
    if (Number.isFinite(d) && d >= 0) {
      durationSec = Math.min(20, d);
    }
  } else if (lowerName === 'fade ui' || lowerName === 'hud fade') {
    const a = Number(ev.value1.trim());
    if (Number.isFinite(a) && a >= 0) {
      const clamped = Math.min(1, a);
      hudAlpha = clamped;
      arrowsAlpha = clamped;
    }
    const d = Number(ev.value2.trim());
    if (Number.isFinite(d) && d >= 0) {
      durationSec = Math.min(20, d);
    }
  }

  for (const src of [ev.value1, ev.value2]) {
    for (const token of src.split(';')) {
      const idx = token.indexOf('=');
      if (idx <= 0) continue;
      const k = token.slice(0, idx).trim().toLowerCase();
      const v = token.slice(idx + 1).trim();
      if (!v) continue;

      if (k === 'action' || k === 'mode') {
        const m = v.toLowerCase();
        if (m === 'reveal') action = 'show';
        else if (m === 'intro' || m === 'hide' || m === 'show' || m === 'toggle' || m === 'fade') action = m;
      } else if (k === 'black' || k === 'blackout') {
        const num = Number(v);
        if (Number.isFinite(num)) blackAlpha = num < 0 ? -1 : Math.min(1, num);
        else {
          const b = parseOptionalBoolFlag(v);
          if (b !== null) blackAlpha = b ? 1 : 0;
        }
      } else if (k === 'hud') {
        const num = Number(v);
        if (Number.isFinite(num)) hudAlpha = num < 0 ? -1 : Math.min(1, num);
        else {
          const b = parseOptionalBoolFlag(v);
          if (b !== null) hudAlpha = b ? 1 : 0;
        }
      } else if (k === 'arrows' || k === 'strums' || k === 'flechas') {
        const num = Number(v);
        if (Number.isFinite(num)) arrowsAlpha = num < 0 ? -1 : Math.min(1, num);
        else {
          const b = parseOptionalBoolFlag(v);
          if (b !== null) arrowsAlpha = b ? 1 : 0;
        }
      } else if (k === 'dur' || k === 'duration') {
        const num = Number(v);
        if (Number.isFinite(num) && num >= 0) durationSec = Math.min(25, num);
      } else if (k === 'revealms' || k === 'reveal_ms' || k === 'delayms') {
        const num = Number(v);
        if (Number.isFinite(num) && num >= 0) revealDelayMs = Math.min(60000, num);
      } else if (k === 'onstart' || k === 'on_start') {
        const b = parseOptionalBoolFlag(v);
        if (b !== null) revealOnSongStart = b;
      } else if (k === 'ease') {
        ease = v;
      }
    }
  }

  const isIntroBlackout =
    action === 'intro' || (timeMs <= 120 && blackAlpha >= 0.85 && hudAlpha >= 0 && hudAlpha <= 0.15);

  return {
    isUiBlackoutEvent: true,
    isIntroBlackout,
    action,
    blackAlpha: isIntroBlackout && blackAlpha < 0 ? 1 : blackAlpha,
    hudAlpha,
    arrowsAlpha,
    durationSec: isIntroBlackout && durationSec <= 0.01 ? 0.65 : durationSec,
    revealDelayMs,
    revealOnSongStart: isIntroBlackout ? revealOnSongStart || revealDelayMs > 0 : false,
    ease,
  };
}

/**
 * Tipos canónicos de notas especiales compatibles con `pulse_core::chart::NoteSpecialKind`.
 */
export type NoteSpecialType = 'default' | 'hurt' | 'instakill' | 'dodge' | 'alt' | 'no_anim';

/**
 * Backends gráficos reconocidos por `pulse_core::gfx_backend::RenderBackendKind`.
 */
export type RenderBackendKind = 'canvas2d' | 'gles2' | 'gles3' | 'vulkan';

/**
 * Normaliza cadenas libres de tipo de nota (ej. `Hurt Note`, `mine`, `kill_mine`, `dodge`, `alt`)
 * hacia el identificador canónico de BeatForge Studio.
 */
export function parseSpecialNoteType(raw?: string | null): NoteSpecialType {
  const clean = (raw ?? '').trim().toLowerCase().replace(/[-\s]+/g, '_');
  if (!clean || clean === 'default' || clean === 'normal' || clean === 'gf_sing' || clean === 'gf') {
    return 'default';
  }
  if (
    clean === 'hurt' ||
    clean === 'hurt_note' ||
    clean === 'mine' ||
    clean === 'mina' ||
    clean === 'damage' ||
    clean === 'danger' ||
    clean === 'peligro' ||
    clean === 'x'
  ) {
    return 'hurt';
  }
  if (
    clean === 'instakill' ||
    clean === 'insta_kill' ||
    clean === 'death' ||
    clean === 'death_note' ||
    clean === 'kill' ||
    clean === 'kill_mine' ||
    clean === 'muerte'
  ) {
    return 'instakill';
  }
  if (clean === 'dodge' || clean === 'dodge_note' || clean === 'esquive' || clean === 'warning' || clean === 'parry') {
    return 'dodge';
  }
  if (clean === 'alt' || clean === 'alt_animation' || clean === 'alt_anim' || clean === 'hey') {
    return 'alt';
  }
  if (clean === 'no_anim' || clean === 'no_animation' || clean === 'sin_anim') {
    return 'no_anim';
  }
  if (clean.includes('instakill') || clean.includes('death') || clean.includes('kill')) {
    return 'instakill';
  }
  if (clean.includes('hurt') || clean.includes('mine') || clean.includes('mina')) {
    return 'hurt';
  }
  if (clean.includes('dodge') || clean.includes('esquiv')) {
    return 'dodge';
  }
  if (clean.includes('alt')) {
    return 'alt';
  }
  if (clean.includes('no_anim') || clean.includes('noanim')) {
    return 'no_anim';
  }
  return 'default';
}

/**
 * Convierte un tipo canónico de nota especial a su código binario en `BFGP` v2 (`nf` bits 4..7).
 */
export function noteSpecialTypeToCode(kind: NoteSpecialType): number {
  switch (kind) {
    case 'hurt':
      return 1;
    case 'instakill':
      return 2;
    case 'dodge':
      return 3;
    case 'alt':
      return 4;
    case 'no_anim':
      return 5;
    default:
      return 0;
  }
}

/**
 * Convierte el código numérico almacenado en `nf` bits 4..7 al tipo canónico de nota especial.
 */
export function noteSpecialCodeToType(code: number): NoteSpecialType {
  switch (code & 0xf) {
    case 1:
      return 'hurt';
    case 2:
      return 'instakill';
    case 3:
      return 'dodge';
    case 4:
      return 'alt';
    case 5:
      return 'no_anim';
    default:
      return 'default';
  }
}

/**
 * Indica si una nota debe renderizarse con la flecha especial marcada con una "X" (`hurt` o `instakill`).
 */
export function isHazardXNote(kind: NoteSpecialType | string): boolean {
  const normalized = parseSpecialNoteType(kind);
  return normalized === 'hurt' || normalized === 'instakill';
}

/**
 * Normaliza la clave del backend gráfico seleccionado para el proyecto (`canvas2d`, `gles2`, `gles3`, `vulkan`).
 */
export function normalizeRenderBackendKind(raw?: string | null): RenderBackendKind {
  switch ((raw ?? '').trim().toLowerCase()) {
    case 'gles2':
    case 'opengl_es2':
    case 'opengles2':
      return 'gles2';
    case 'gles3':
    case 'gles':
    case 'opengl_es3':
    case 'opengles3':
      return 'gles3';
    case 'vulkan':
    case 'vk':
      return 'vulkan';
    default:
      return 'canvas2d';
  }
}

export interface ArrowSparrowMatch {
  kind: 'arrow' | 'receptor' | 'hurt';
  direction?: 'left' | 'down' | 'up' | 'right';
}

/**
 * Clasifica un `<SubTexture name="...">` dentro de un atlas `NOTE_assets.xml` estándar de mods
 * (flechas `purple/blue/green/red`, receptores `arrowLEFT/DOWN/UP/RIGHT` y minas/notas X `hurt/mine`),
 * ignorando cuadros auxiliares (`hold`, `tail`, `end`, `piece`, `confirm`, `press`).
 * Espejo exacto de `pulse_core::svg_skin::import_arrow_sparrow_atlas`.
 */
export function matchArrowSparrowSubTexture(subTexName: string): ArrowSparrowMatch | null {
  const low = subTexName.trim().toLowerCase().replace(/[_\-\s0-9]/g, '');
  if (!low) return null;

  const isReceptor =
    low.startsWith('arrow') && !low.includes('confirm') && !low.includes('press');
  if (isReceptor) {
    if (low.includes('left') || low.includes('izq')) return { kind: 'receptor', direction: 'left' };
    if (low.includes('down') || low.includes('abajo')) return { kind: 'receptor', direction: 'down' };
    if (low.includes('up') || low.includes('arriba')) return { kind: 'receptor', direction: 'up' };
    if (low.includes('right') || low.includes('der')) return { kind: 'receptor', direction: 'right' };
    return null;
  }

  if (
    low.includes('hurt') ||
    low.includes('mine') ||
    low.includes('mina') ||
    low.includes('death') ||
    low.includes('kill') ||
    low.includes('danger')
  ) {
    return { kind: 'hurt' };
  }

  const isFx =
    low.includes('hold') ||
    low.includes('tail') ||
    low.includes('end') ||
    low.includes('piece') ||
    low.includes('confirm') ||
    low.includes('press');
  if (isFx) {
    return null;
  }

  if (low === 'purple' || low.includes('purple') || low.includes('left') || low.includes('izq')) {
    return { kind: 'arrow', direction: 'left' };
  }
  if (low === 'blue' || low.includes('blue') || low.includes('down') || low.includes('abajo')) {
    return { kind: 'arrow', direction: 'down' };
  }
  if (low === 'green' || low.includes('green') || low.includes('up') || low.includes('arriba')) {
    return { kind: 'arrow', direction: 'up' };
  }
  if (low === 'red' || low.includes('red') || low.includes('right') || low.includes('der')) {
    return { kind: 'arrow', direction: 'right' };
  }
  return null;
}

export interface BfgpExtensionFlags {
  gameOver: boolean;
  gameOverPhase: number;
  botplay: boolean;
  practiceMode: boolean;
}

/**
 * Decodifica los bits extendidos `9..13` del campo `flags` en la cabecera `BFGP` v2 (`152B`):
 * bit 9 = `gameOver`, bits 10..11 = `gameOverPhase` (`0..3`), bit 12 = `botplay`, bit 13 = `practiceMode`.
 */
export function decodeBfgpExtensionFlags(flags: number): BfgpExtensionFlags {
  return {
    gameOver: (flags & (1 << 9)) !== 0,
    gameOverPhase: (flags >>> 10) & 0x3,
    botplay: (flags & (1 << 12)) !== 0,
    practiceMode: (flags & (1 << 13)) !== 0,
  };
}

/**
 * Extrae el código de nota especial (`0..5`) codificado en `noteFlags` bits 4..7 de cada nota visible `BFGP` v2.
 */
export function decodeBfgpNoteKindCode(noteFlags: number): number {
  return (noteFlags >>> 4) & 0xf;
}

export interface AtlasFrameBox {
  width: number;
  height: number;
  frameX: number;
  frameY: number;
  frameWidth: number;
  frameHeight: number;
  rotated?: boolean;
}

export interface NormalizedAtlasFramesResult {
  unifiedWidth: number;
  unifiedHeight: number;
  animations: Record<string, AtlasFrameBox[]>;
}

/**
 * Centra horizontalmente y alinea a la base todas las animaciones de un atlas dentro de un
 * lienzo lógico unificado (`frameWidth x frameHeight`), preservando el movimiento relativo
 * entre frames de cada animación y normalizando frames sin recorte.
 * Espejo exacto de `pulse_core::sparrow::SparrowAtlas::normalize_and_center_animations`.
 */
export function normalizeAndCenterAtlasFrames(
  animations: Record<string, AtlasFrameBox[]>,
): NormalizedAtlasFramesResult {
  let unifiedW = 1;
  let unifiedH = 1;

  interface AnimLayout {
    prefix: string;
    spanW: number;
    spanH: number;
    relOffsets: Array<[number, number]>;
  }

  const layouts: AnimLayout[] = [];
  for (const [prefix, frames] of Object.entries(animations)) {
    if (!frames || frames.length === 0) continue;

    const visDims = frames.map((f): [number, number] => [
      Math.max(1, f.rotated ? f.height : f.width),
      Math.max(1, f.rotated ? f.width : f.height),
    ]);

    const hasOffsets = frames.some(
      (f, idx) =>
        f.frameX !== 0 ||
        f.frameY !== 0 ||
        f.frameWidth > visDims[idx][0] ||
        f.frameHeight > visDims[idx][1],
    );

    let spanW = 1;
    let spanH = 1;
    let relOffsets: Array<[number, number]> = [];

    if (!hasOffsets) {
      for (const [vw, vh] of visDims) {
        spanW = Math.max(spanW, vw);
        spanH = Math.max(spanH, vh);
      }
      relOffsets = visDims.map(([vw, vh]) => [
        Math.trunc(Math.max(0, spanW - vw) / 2),
        Math.max(0, spanH - vh),
      ]);
    } else {
      let minX = Number.POSITIVE_INFINITY;
      let minY = Number.POSITIVE_INFINITY;
      let maxX = Number.NEGATIVE_INFINITY;
      let maxY = Number.NEGATIVE_INFINITY;
      const rawCoords: Array<[number, number]> = [];

      for (let i = 0; i < frames.length; i += 1) {
        const f = frames[i];
        const [vw, vh] = visDims[i];
        const vx0 = -f.frameX;
        const vy0 = -f.frameY;
        const vx1 = vx0 + vw;
        const vy1 = vy0 + vh;
        minX = Math.min(minX, vx0);
        minY = Math.min(minY, vy0);
        maxX = Math.max(maxX, vx1);
        maxY = Math.max(maxY, vy1);
        rawCoords.push([vx0, vy0]);
      }

      spanW = Math.max(1, maxX - minX);
      spanH = Math.max(1, maxY - minY);
      relOffsets = rawCoords.map(([vx0, vy0]) => [vx0 - minX, vy0 - minY]);
    }

    unifiedW = Math.max(unifiedW, spanW);
    unifiedH = Math.max(unifiedH, spanH);
    layouts.push({ prefix, spanW, spanH, relOffsets });
  }

  const out: Record<string, AtlasFrameBox[]> = {};
  for (const [prefix, frames] of Object.entries(animations)) {
    out[prefix] = frames.map((f) => ({ ...f }));
  }

  for (const layout of layouts) {
    const frames = out[layout.prefix];
    if (!frames) continue;
    const padX = Math.trunc(Math.max(0, unifiedW - layout.spanW) / 2);
    const padY = Math.max(0, unifiedH - layout.spanH);
    for (let i = 0; i < frames.length; i += 1) {
      const [rx, ry] = layout.relOffsets[i];
      const fx = -(rx + padX);
      const fy = -(ry + padY);
      frames[i].frameX = fx === 0 ? 0 : fx;
      frames[i].frameY = fy === 0 ? 0 : fy;
      frames[i].frameWidth = unifiedW;
      frames[i].frameHeight = unifiedH;
    }
  }

  return {
    unifiedWidth: unifiedW,
    unifiedHeight: unifiedH,
    animations: out,
  };
}

export interface StageImportDimensionParams {
  role: 'background' | 'player' | 'opponent' | 'decoration';
  width: number;
  height: number;
  isAtlas?: boolean;
  idleHeight?: number;
}

/**
 * Calcula las dimensiones lógicas (`1280x720`) de una entidad al importar PNG/ASTC o atlas XML/JSON
 * en escenario, evitando que hojas completas o sprites pequeños queden diminutos o aplastados.
 * Espejo exacto de `StageEntity::fit_dimensions_to_atlas` / `fit_dimensions_to_image`.
 */
export function computeStageImportDimensions(
  params: StageImportDimensionParams,
): { width: number; height: number } {
  const w = Math.max(1, params.width);
  const h = Math.max(1, params.height);

  if (params.isAtlas) {
    const idleH = Math.max(1, params.idleHeight ?? h);
    if (params.role === 'background') {
      const cover = Math.max(1280 / w, 720 / h);
      return {
        width: Math.min(2560, Math.max(1280, w * cover)),
        height: Math.min(1440, Math.max(720, h * cover)),
      };
    }
    if (params.role === 'player' || params.role === 'opponent') {
      const fit = Math.min(380 / idleH, 640 / w, 520 / h);
      return {
        width: Math.max(140, w * fit),
        height: Math.max(160, h * fit),
      };
    }
    const fit = Math.min(320 / idleH, 640 / w, 460 / h);
    return {
      width: Math.max(120, w * fit),
      height: Math.max(120, h * fit),
    };
  }

  if (params.role === 'background') {
    const cover = Math.max(1280 / w, 720 / h);
    return {
      width: Math.min(2560, Math.max(1280, w * cover)),
      height: Math.min(1440, Math.max(720, h * cover)),
    };
  }
  if (params.role === 'player' || params.role === 'opponent') {
    const fit = Math.min(520 / w, 380 / h);
    return {
      width: Math.min(560, Math.max(140, w * fit)),
      height: Math.min(460, Math.max(160, h * fit)),
    };
  }
  const fit = Math.min(560 / w, 320 / h);
  return {
    width: Math.min(640, Math.max(120, w * fit)),
    height: Math.min(420, Math.max(120, h * fit)),
  };
}






