/**
 * Archivo: sdk/ts/src/engine-tools.ts
 * Propósito: Herramientas modulares en TypeScript para validar charts JSON,
 * probar el algoritmo de auto-armado Sparrow XML, validar el límite de 1 a 3
 * pistas de audio y simular el descarte de objetos invisibles del compilador en caliente.
 */

export type NoteDirection = 'left' | 'down' | 'up' | 'right';
export type EntityRole = 'background' | 'player' | 'opponent' | 'decoration';

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
  notes: ParsedChartNote[];
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
  const notes: ParsedChartNote[] = [];

  for (const sec of song.notes) {
    const mustHit = Boolean(sec?.mustHitSection ?? true);
    const secNotes = Array.isArray(sec?.sectionNotes) ? sec.sectionNotes : [];
    for (const item of secNotes) {
      if (!Array.isArray(item) || item.length < 2) continue;
      const timeMs = Math.max(0, Number(item[0]) || 0);
      const rawLane = Number(item[1]);
      if (!Number.isFinite(rawLane) || rawLane < 0) continue;
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
  return { title, bpm, speed, notes };
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
