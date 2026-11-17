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

export type ScriptLang = 'lua' | 'js' | 'ts';

export interface ScriptLoopOptResult {
  optimizedSource: string;
  discardedLoops: number;
  neutralizedLoops: number;
}

const NECESSARY_ENGINE_APIS = [
  'setEntityProp',
  'setCameraZoom',
  'shakeCamera',
  'playAnim',
  'getSongPosMs',
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
export const BINARY_GAMEPLAY_HEADER_BYTES = 112;
export const BINARY_GAMEPLAY_ENTITY_BYTES = 60;
export const BINARY_GAMEPLAY_NOTE_BYTES = 28;

export interface DecodedBinaryEntity {
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  visible: boolean;
  flipX: boolean;
  hasFrame: boolean;
  frameRotated: boolean;
  roleCode: number;
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

export interface DecodedBinaryGameplayFrame {
  active: boolean;
  downscroll: boolean;
  paused: boolean;
  finished: boolean;
  controlMode: MobileControlMode;
  hasHudStrings: boolean;
  hudRevision: number;
  holdingLanesMask: number;
  opponentHoldingLanesMask: number;
  countdownStage: number;
  countdownProgress: number;
  songPosMs: number;
  durationMs: number;
  scrollSpeed: number;
  activeBpm: number;
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
  entities: DecodedBinaryEntity[];
  visibleNotes: DecodedBinaryNote[];
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
  const entityCount = view.getUint16(108, true);
  const noteCount = view.getUint16(110, true);

  let cursor = BINARY_GAMEPLAY_HEADER_BYTES;
  const requiredMin =
    cursor +
    entityCount * BINARY_GAMEPLAY_ENTITY_BYTES +
    noteCount * BINARY_GAMEPLAY_NOTE_BYTES;
  if (bytes.byteLength < requiredMin) {
    throw new Error(`Buffer insuficiente para entidades/notas (${bytes.byteLength} < ${requiredMin})`);
  }

  const entities: DecodedBinaryEntity[] = [];
  for (let i = 0; i < entityCount; i++) {
    const x = view.getFloat32(cursor, true);
    const y = view.getFloat32(cursor + 4, true);
    const width = view.getFloat32(cursor + 8, true);
    const height = view.getFloat32(cursor + 12, true);
    const scale = view.getFloat32(cursor + 16, true);
    const opacity = view.getFloat32(cursor + 20, true);
    const entFlags = view.getUint32(cursor + 24, true);
    const atlasX = view.getUint32(cursor + 28, true);
    const atlasY = view.getUint32(cursor + 32, true);
    const atlasW = view.getUint32(cursor + 36, true);
    const atlasH = view.getUint32(cursor + 40, true);
    const offX = view.getInt32(cursor + 44, true);
    const offY = view.getInt32(cursor + 48, true);
    const logicalW = view.getUint32(cursor + 52, true);
    const logicalH = view.getUint32(cursor + 56, true);
    cursor += BINARY_GAMEPLAY_ENTITY_BYTES;

    entities.push({
      x,
      y,
      width,
      height,
      scale,
      opacity,
      visible: (entFlags & 1) !== 0,
      flipX: (entFlags & (1 << 1)) !== 0,
      hasFrame: (entFlags & (1 << 2)) !== 0,
      frameRotated: (entFlags & (1 << 3)) !== 0,
      roleCode: (entFlags >> 4) & 3,
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

  const hasHudStrings = (flags & (1 << 5)) !== 0;
  let lastJudgement: string | undefined;
  let countdownLabel: string | undefined;
  let scriptError: string | undefined;
  let scriptLog: string | undefined;

  if (hasHudStrings) {
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
    hudRevision,
    holdingLanesMask: masks & 0xff,
    opponentHoldingLanesMask: (masks >> 8) & 0xff,
    countdownStage,
    countdownProgress,
    songPosMs,
    durationMs,
    scrollSpeed,
    activeBpm,
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
    entities,
    visibleNotes,
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

