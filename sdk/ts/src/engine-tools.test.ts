/**
 * Archivo: sdk/ts/src/engine-tools.test.ts
 * Propósito: Suite de pruebas unitarias (`node:test`) que valida las reglas
 * clave del motor: parseo/serialización de `chart.json` completo, cuenta regresiva BPM,
 * cabecera `.astc`, detección de formatos `.json` (TexturePacker / Animate),
 * auto-armado Sparrow, límite de 3 pistas y descarte de assets invisibles.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  BINARY_GAMEPLAY_ENTITY_BYTES,
  BINARY_GAMEPLAY_HEADER_BYTES,
  BINARY_GAMEPLAY_MAGIC,
  BINARY_GAMEPLAY_NOTE_BYTES,
  LruResourceCache,
  advanceNoteSlidingWindow,
  autoBindSparrowPrefixes,
  clampEntityScale,
  compareNaturalFrameNames,
  computeCountdownState,
  computeReceptorLayout,
  computeSmartTextureScale,
  computeSustainTailSpan,
  computeTrimmedFramePlacement,
  computeUniformCharacterDrawBox,
  decodeBinaryGameplayFrame,
  desaturateToNeutralRgba,
  detectSpriteJsonFormat,
  mapRotatedSpriteSample,
  optimizeScriptLoops,
  packPosesIntoBoundedAtlas,
  parseAstcHeader,
  parseChartJson,
  resolveMobileTouchLanes,
  serializeChartJson,
  shouldDiscardInvisibleEntity,
  sortProjectSummaries,
  stripTrailingFrameDigits,
  validateAudioTrackCount,
} from './engine-tools';

test('parseChartJson normalizes mustHitSection player and opponent lanes', () => {
  const raw = JSON.stringify({
    song: {
      song: 'Test Track',
      bpm: 140,
      speed: 2.2,
      notes: [
        {
          mustHitSection: true,
          sectionNotes: [
            [1000, 0, 0], // player left
            [1250, 6, 150], // opponent up
          ],
        },
        {
          mustHitSection: false,
          sectionNotes: [
            [2000, 1, 0], // opponent down
            [2250, 7, 0], // player right
          ],
        },
      ],
    },
  });

  const parsed = parseChartJson(raw);
  assert.equal(parsed.title, 'Test Track');
  assert.equal(parsed.bpm, 140);
  assert.equal(parsed.notes.length, 4);
  assert.deepEqual(parsed.notes[0], {
    timeMs: 1000,
    direction: 'left',
    isPlayer: true,
    sustainMs: 0,
  });
  assert.deepEqual(parsed.notes[1], {
    timeMs: 1250,
    direction: 'up',
    isPlayer: false,
    sustainMs: 150,
  });
  assert.equal(parsed.notes[2].isPlayer, false);
  assert.equal(parsed.notes[3].isPlayer, true);
});

test('parseChartJson and serializeChartJson preserve full chart.json metadata, changeBPM and events', () => {
  const raw = JSON.stringify({
    song: {
      song: 'Neon Pulse',
      bpm: 125,
      speed: 2.5,
      needsVoices: false,
      player1: 'hero_rig',
      player2: 'rival_rig',
      gfVersion: 'speaker_rig',
      stage: 'rooftop_night',
      arrowSkin: 'custom_arrows',
      splashSkin: 'custom_splash',
      validScore: true,
      events: [[2000, [['Add Camera Zoom', '0.03', '0.02']]]],
      notes: [
        {
          mustHitSection: true,
          sectionBeats: 4,
          changeBPM: false,
          bpm: 125,
          sectionNotes: [
            [500, 2, 0],
            [750, -1, 'Play Animation', 'hey', 'player'],
          ],
        },
        {
          mustHitSection: false,
          sectionBeats: 4,
          changeBPM: true,
          bpm: 160,
          altAnim: true,
          gfSection: true,
          sectionNotes: [[2100, 4, 240]],
        },
      ],
    },
  });

  const parsed = parseChartJson(raw);
  assert.equal(parsed.needsVoices, false);
  assert.equal(parsed.player1, 'hero_rig');
  assert.equal(parsed.player2, 'rival_rig');
  assert.equal(parsed.stage, 'rooftop_night');
  assert.equal(parsed.events.length, 2);
  assert.equal(parsed.events[0].name, 'Play Animation');
  assert.equal(parsed.events[1].name, 'Add Camera Zoom');
  assert.equal(parsed.sections[1].changeBpm, true);
  assert.equal(parsed.sections[1].bpm, 160);
  assert.equal(parsed.sections[1].altAnim, true);
  assert.equal(parsed.notes[1].isPlayer, true);

  const reserialized = serializeChartJson(parsed);
  const roundTripped = parseChartJson(reserialized);
  assert.equal(roundTripped.player1, 'hero_rig');
  assert.equal(roundTripped.events.length, 2);
  assert.equal(roundTripped.sections[1].bpm, 160);
  assert.equal(roundTripped.notes[1].sustainMs, 240);
});

test('computeCountdownState maps 4 BPM-synced beats to 3, 2, 1, ¡YA!', () => {
  // BPM 120 -> beatMs = 500ms, inicio en -2000ms
  const s0 = computeCountdownState(-1900, 120);
  assert.equal(s0.active, true);
  assert.equal(s0.stage, 0);
  assert.equal(s0.label, '3');

  const s1 = computeCountdownState(-1400, 120);
  assert.equal(s1.stage, 1);
  assert.equal(s1.label, '2');

  const s2 = computeCountdownState(-800, 120);
  assert.equal(s2.stage, 2);
  assert.equal(s2.label, '1');

  const s3 = computeCountdownState(-250, 120);
  assert.equal(s3.stage, 3);
  assert.equal(s3.label, '¡YA!');
  assert.ok(Math.abs(s3.progress - 0.5) < 0.001);

  const done = computeCountdownState(0, 120);
  assert.equal(done.active, false);
  assert.equal(done.stage, -1);
});

test('parseAstcHeader validates 16-byte header, block dims and payload size', () => {
  // 256x128 con bloques 8x8 => 32x16 bloques * 16 bytes = 8192 bytes payload
  const header = new Uint8Array([
    0x13, 0xab, 0xa1, 0x5c,
    8, 8, 1,
    0x00, 0x01, 0x00, // 256
    0x80, 0x00, 0x00, // 128
    0x01, 0x00, 0x00, // 1
  ]);
  const info = parseAstcHeader(header);
  assert.equal(info.blockDimX, 8);
  assert.equal(info.blockDimY, 8);
  assert.equal(info.width, 256);
  assert.equal(info.height, 128);
  assert.equal(info.expectedPayloadBytes, 8192);
  assert.equal(info.totalExpectedBytes, 8208);

  assert.throws(() => parseAstcHeader(new Uint8Array([0, 1, 2, 3])));
});

test('detectSpriteJsonFormat classifies TexturePacker, spritemap1.json and Animation.json', () => {
  assert.equal(
    detectSpriteJsonFormat(JSON.stringify({ frames: { 'idle0000': { frame: { x: 0, y: 0, w: 64, h: 64 } } } })),
    'texture_packer_hash',
  );
  assert.equal(
    detectSpriteJsonFormat(JSON.stringify({ frames: [{ filename: 'idle0000', frame: { x: 0, y: 0, w: 64, h: 64 } }] })),
    'texture_packer_array',
  );
  assert.equal(
    detectSpriteJsonFormat(JSON.stringify({ ATLAS: { SPRITES: [{ SPRITE: { name: '0', x: 0, y: 0, w: 32, h: 32 } }] } })),
    'animate_spritemap',
  );
  assert.equal(
    detectSpriteJsonFormat(JSON.stringify({ AN: { N: 'idle', TL: { L: [] } }, SD: { S: [] } })),
    'animate_animation',
  );
});

test('computeTrimmedFramePlacement un-rotates dimensions and applies negative frameX/frameY offsets', () => {
  const placed = computeTrimmedFramePlacement({
    width: 360,
    height: 160,
    frameX: -20,
    frameY: -30,
    frameWidth: 200,
    frameHeight: 400,
    rotated: true,
  });
  assert.equal(placed.visualWidth, 160);
  assert.equal(placed.visualHeight, 360);
  assert.equal(placed.logicalWidth, 200);
  assert.equal(placed.logicalHeight, 400);
  assert.equal(placed.offsetX, 20);
  assert.equal(placed.offsetY, 30);
});

test('autoBindSparrowPrefixes maps Idle, Left, Down, Up, Right automatically', () => {
  const bindings = autoBindSparrowPrefixes([
    'Hero_idle_dance',
    'Hero_singLEFT',
    'Hero_singDOWN',
    'Hero_singUP',
    'Hero_singRIGHT',
    'Hero_singLEFT_miss',
  ]);

  assert.equal(bindings.idle, 'Hero_idle_dance');
  assert.equal(bindings.left, 'Hero_singLEFT');
  assert.equal(bindings.down, 'Hero_singDOWN');
  assert.equal(bindings.up, 'Hero_singUP');
  assert.equal(bindings.right, 'Hero_singRIGHT');
});

test('validateAudioTrackCount enforces 1 to 3 audio files limit', () => {
  assert.equal(validateAudioTrackCount(['mix.ogg']), 1);
  assert.equal(validateAudioTrackCount(['inst.ogg', 'voices.ogg']), 2);
  assert.equal(validateAudioTrackCount(['inst.ogg', 'player.ogg', 'rival.ogg']), 3);
  assert.throws(() => validateAudioTrackCount(['a.ogg', 'b.ogg', 'c.ogg', 'd.ogg']));
});

test('shouldDiscardInvisibleEntity culls permanently invisible assets unless referenced by script', () => {
  const hiddenProp = {
    id: 'prop_hidden',
    role: 'decoration' as const,
    x: 640,
    y: 360,
    width: 200,
    height: 200,
    scale: 1,
    opacity: 0,
    visible: true,
    zIndex: 5,
  };

  assert.equal(shouldDiscardInvisibleEntity(hiddenProp, []), true);
  assert.equal(
    shouldDiscardInvisibleEntity(hiddenProp, ['setEntityProp("prop_hidden", "opacity", 1.0)']),
    false,
  );
});

test('mapRotatedSpriteSample inverts +90 deg CW packed piece coordinates accurately', () => {
  // Original visual piece 4x8 rotated +90 CW -> packed rect 8x4 (w=8, h=4)
  // Top-left pixel center (lx=0.5, ly=0.5) maps to right edge (u=7, v=0)
  const topLeft = mapRotatedSpriteSample(0.5, 0.5, 8, 4, true);
  assert.equal(topLeft.u, 7);
  assert.equal(topLeft.v, 0);

  // Bottom-right pixel center (lx=3.5, ly=7.5) maps to (u=0, v=3)
  const bottomRight = mapRotatedSpriteSample(3.5, 7.5, 8, 4, true);
  assert.equal(bottomRight.u, 0);
  assert.equal(bottomRight.v, 3);
});

test('stripTrailingFrameDigits and compareNaturalFrameNames sort TexturePacker and Sparrow frames naturally', () => {
  assert.equal(stripTrailingFrameDigits('hero_idle0004.png'), 'hero_idle');
  assert.equal(stripTrailingFrameDigits('spritemap_up12.astc'), 'spritemap_up');

  const sorted = ['idle10.png', 'idle2.png', 'idle1.png', 'idle0.png'].sort(compareNaturalFrameNames);
  assert.deepEqual(sorted, ['idle0.png', 'idle1.png', 'idle2.png', 'idle10.png']);
});

test('packPosesIntoBoundedAtlas deduplicates identical poses and stays within 4096x4096', () => {
  const poses = Array.from({ length: 180 }, (_, i) => ({
    signature: `pose_${i % 28}`,
    width: 480,
    height: 540,
  }));
  const packed = packPosesIntoBoundedAtlas(poses, 4096, 2);
  assert.equal(packed.uniqueCount, 28);
  assert.equal(packed.frameToPoseIndex.length, 180);
  assert.ok(packed.sheetWidth <= 4096);
  assert.ok(packed.sheetHeight <= 4096);
});

test('computeUniformCharacterDrawBox preserves character aspect ratio and bottom-center anchor', () => {
  const res = computeUniformCharacterDrawBox(
    { cx: 940, top: 210, w: 320, h: 320 },
    { width: 250, height: 500, frameWidth: 250, frameHeight: 500, rotated: false },
  );
  assert.ok(Math.abs(res.boxW - 160) < 0.01);
  assert.ok(Math.abs(res.boxH - 320) < 0.01);
  assert.ok(Math.abs(res.boxLeft - (940 - 80)) < 0.01);
  assert.ok(Math.abs(res.boxTop - 210) < 0.01);
});

test('parseChartJson supports V-Slice difficulty map, UTF-8 BOM, double-wrapped song.song and Codename strumLines', () => {
  const vslice = `\uFEFF${JSON.stringify({
    version: '2.0.0',
    songName: 'V-Slice Track',
    bpm: 150,
    scrollSpeed: { hard: 2.8, normal: 2.2 },
    events: [{ t: 1200, e: 'FocusCamera', v: { char: 1 } }],
    notes: {
      hard: [
        { t: 500, d: 0, l: 0, k: '' },
        { t: 1000, d: 5, l: 200, k: 'alt' },
      ],
    },
  })}`;
  const parsedV = parseChartJson(vslice);
  assert.equal(parsedV.title, 'V-Slice Track');
  assert.equal(parsedV.bpm, 150);
  assert.equal(parsedV.speed, 2.8);
  assert.equal(parsedV.notes.length, 2);
  assert.equal(parsedV.notes[0].isPlayer, true);
  assert.equal(parsedV.notes[0].direction, 'left');
  assert.equal(parsedV.notes[1].isPlayer, false);
  assert.equal(parsedV.notes[1].direction, 'down');
  assert.equal(parsedV.notes[1].sustainMs, 200);
  assert.equal(parsedV.events.length, 1);
  assert.ok(parsedV.sections.length >= 4);

  const doubleWrapped = JSON.stringify({
    song: {
      song: {
        song: 'Double Wrapped',
        bpm: '140',
        notes: [{ mustHit: true, sectionNotes: [[400, 2, 0]] }],
      },
    },
  });
  const parsedDw = parseChartJson(doubleWrapped);
  assert.equal(parsedDw.title, 'Double Wrapped');
  assert.equal(parsedDw.bpm, 140);
  assert.equal(parsedDw.notes.length, 1);
  assert.equal(parsedDw.notes[0].isPlayer, true);

  const codename = JSON.stringify({
    codenameChart: true,
    bpm: 132,
    scrollSpeed: 2.4,
    events: [{ name: 'Camera Movement', time: 800, params: [1] }],
    strumLines: [
      { type: 0, position: 'dad', notes: [{ time: 500, id: 1, sLen: 0 }] },
      { type: 1, position: 'boyfriend', notes: [{ time: 1000, id: 3, sLen: 150 }] },
    ],
  });
  const parsedCn = parseChartJson(codename);
  assert.equal(parsedCn.bpm, 132);
  assert.equal(parsedCn.speed, 2.4);
  assert.equal(parsedCn.notes.length, 2);
  assert.equal(parsedCn.notes[0].isPlayer, false);
  assert.equal(parsedCn.notes[0].direction, 'down');
  assert.equal(parsedCn.notes[1].isPlayer, true);
  assert.equal(parsedCn.notes[1].direction, 'right');
  assert.equal(parsedCn.notes[1].sustainMs, 150);
  assert.equal(parsedCn.events.length, 1);
});

test('optimizeScriptLoops discards useless heavy loops and preserves/neutralizes necessary logic in Lua, JS and TS', () => {
  const tsRes = optimizeScriptLoops(
    'ts',
    `
function onUpdate(dt: number) {
  let acc = 0;
  for (let i = 0; i < 1000000; i++) { acc += i; }
  while (true) { setCameraZoom(1.06); }
}
`,
  );
  assert.equal(tsRes.discardedLoops, 1);
  assert.equal(tsRes.neutralizedLoops, 1);
  assert.ok(!tsRes.optimizedSource.includes('1000000'));
  assert.ok(!tsRes.optimizedSource.includes('while (true)'));
  assert.ok(tsRes.optimizedSource.includes('setCameraZoom(1.06)'));

  const luaRes = optimizeScriptLoops(
    'lua',
    `
function onUpdate(dt)
  while true do x = x + 1 end
  for i = 1, 50000 do
    setEntityProp("stage_bg", "opacity", 0.9)
  end
end
`,
  );
  assert.equal(luaRes.discardedLoops, 1);
  assert.equal(luaRes.neutralizedLoops, 1);
  assert.ok(luaRes.optimizedSource.includes('1, 64'));
  assert.ok(luaRes.optimizedSource.includes('setEntityProp'));
});

test('computeSmartTextureScale passes through optimal sprites without quality loss and downscales oversized ones', () => {
  const optimal = computeSmartTextureScale({
    origWidth: 1920,
    origHeight: 1080,
    entityWidth: 1280,
    entityHeight: 720,
    entityScale: 1.0,
    hasAtlas: false,
    tier: 'balanced',
  });
  assert.equal(optimal, 1.0);

  const oversized = computeSmartTextureScale({
    origWidth: 4096,
    origHeight: 4096,
    entityWidth: 200,
    entityHeight: 200,
    entityScale: 1.0,
    hasAtlas: false,
    tier: 'balanced',
  });
  assert.ok(oversized < 0.6);
});

test('advanceNoteSlidingWindow and LruResourceCache discard resolved notes and evict oldest textures deterministically', () => {
  const notes = [
    { timeMs: 400, sustainMs: 0 },
    { timeMs: 800, sustainMs: 0 },
    { timeMs: 1600, sustainMs: 0 },
  ];
  const win = advanceNoteSlidingWindow(notes, ['hit', 'missed', 'pending'], 0, 1200, 240);
  assert.equal(win.nextStartIdx, 2);
  assert.equal(win.newlyDiscarded, 2);

  const cache = new LruResourceCache<string>(1024, 10);
  cache.insert('a', 'bufA', 500);
  cache.insert('b', 'bufB', 500);
  assert.equal(cache.get('a'), 'bufA'); // a es más reciente que b
  const evicted = cache.insert('c', 'bufC', 500);
  assert.deepEqual(evicted, ['b']);
  assert.equal(cache.has('a'), true);
  assert.equal(cache.has('c'), true);
  assert.equal(cache.bytesUsed, 1000);
});

test('desaturateToNeutralRgba removes vivid arrow color for uncolored receptors', () => {
  // Píxel rosa vibrante (Left) y cian vibrante (Down) -> gris neutro
  const neutral = desaturateToNeutralRgba([244, 114, 182, 255, 34, 211, 238, 255]);
  const chroma0 = Math.max(neutral[0], neutral[1], neutral[2]) - Math.min(neutral[0], neutral[1], neutral[2]);
  const chroma1 = Math.max(neutral[4], neutral[5], neutral[6]) - Math.min(neutral[4], neutral[5], neutral[6]);
  assert.ok(chroma0 <= 6);
  assert.ok(chroma1 <= 6);
});

test('computeSustainTailSpan shrinks tail smoothly into receptorY after head hit without disappearing', () => {
  // Upscroll: nota larga de 600ms en t=1000ms, estamos en t=1200ms con cabeza ya golpeada
  const upscrollMid = computeSustainTailSpan({
    noteTimeMs: 1000,
    sustainMs: 600,
    songPosMs: 1200,
    headHit: true,
    downscroll: false,
    receptorY: 118,
    pxPerMs: 0.5,
  });
  assert.equal(upscrollMid.headVisible, false);
  assert.equal(upscrollMid.tailTop, 118);
  assert.equal(upscrollMid.tailLength, 200); // 400ms restantes * 0.5

  // Downscroll: misma nota en t=1500ms (quedan 100ms = 50px arriba del receptor)
  const downscrollEnd = computeSustainTailSpan({
    noteTimeMs: 1000,
    sustainMs: 600,
    songPosMs: 1500,
    headHit: true,
    downscroll: true,
    receptorY: 575,
    pxPerMs: 0.5,
  });
  assert.equal(downscrollEnd.headVisible, false);
  assert.equal(downscrollEnd.tailBottom, 575);
  assert.equal(downscrollEnd.tailLength, 50);
});

test('resolveMobileTouchLanes uses wide bottom layout ONLY in downscroll tap_arrows and standard layout in upscroll', () => {
  const downLayout = computeReceptorLayout({
    controlMode: 'tap_arrows',
    downscroll: true,
    screenWidth: 1280,
    screenHeight: 720,
  });
  assert.equal(downLayout.downscrollWideTactile, true);
  assert.deepEqual(downLayout.playerXs, [340, 495, 785, 940]);
  assert.equal(downLayout.playerY, 595);
  assert.equal(downLayout.opponentY, 100);

  const upLayout = computeReceptorLayout({
    controlMode: 'tap_arrows',
    downscroll: false,
    screenWidth: 1280,
    screenHeight: 720,
  });
  assert.equal(upLayout.downscrollWideTactile, false);
  assert.deepEqual(upLayout.playerXs, [765, 870, 975, 1080]);
  assert.equal(upLayout.playerY, 118);

  // Downscroll tap_arrows: tocar flechas separadas abajo (IZQ en 340, DER en 940)
  const hitDownscrollBottom = resolveMobileTouchLanes({
    controlMode: 'tap_arrows',
    touches: [
      { x: 342, y: 595 },
      { x: 938, y: 600 },
    ],
    screenWidth: 1280,
    screenHeight: 720,
    downscroll: true,
  });
  assert.equal(hitDownscrollBottom, (1 << 0) | (1 << 3));

  // Upscroll tap_arrows: tocar fuera de las flechas superiores no cuenta
  const missOutside = resolveMobileTouchLanes({
    controlMode: 'tap_arrows',
    touches: [{ x: 340, y: 595 }],
    screenWidth: 1280,
    screenHeight: 720,
    downscroll: false,
  });
  assert.equal(missOutside, 0);

  const hitUpscrollTop = resolveMobileTouchLanes({
    controlMode: 'tap_arrows',
    touches: [
      { x: 765, y: 120 },
      { x: 1080, y: 115 },
    ],
    screenWidth: 1280,
    screenHeight: 720,
    downscroll: false,
  });
  assert.equal(hitUpscrollTop, (1 << 0) | (1 << 3));

  // Hitbox clásico (Psych Mobile): 4 columnas verticales completas sin importar Y
  const hitboxTouch = resolveMobileTouchLanes({
    controlMode: 'classic_hitbox',
    touches: [
      { x: 400, y: 600 },
      { x: 1150, y: 200 },
    ],
    screenWidth: 1280,
    screenHeight: 720,
    downscroll: true,
  });
  assert.equal(hitboxTouch, (1 << 1) | (1 << 3));
});

test('decodeBinaryGameplayFrame decodes BFGP header, scaled entities, visible notes and optional HUD strings', () => {
  const encoder = new TextEncoder();
  const sJudg = encoder.encode('¡PERFECTO!');
  const sCd = encoder.encode('¡YA!');
  const sErr = encoder.encode('');
  const sLog = encoder.encode('onBeatHit(4)');

  const totalBytes =
    BINARY_GAMEPLAY_HEADER_BYTES +
    BINARY_GAMEPLAY_ENTITY_BYTES +
    BINARY_GAMEPLAY_NOTE_BYTES +
    8 +
    sJudg.length +
    sCd.length +
    sErr.length +
    sLog.length;

  const buf = new Uint8Array(totalBytes);
  const view = new DataView(buf.buffer);

  // Header 112B
  view.setUint32(0, BINARY_GAMEPLAY_MAGIC, true);
  view.setUint32(4, 1 | (1 << 1) | (1 << 4) | (1 << 5), true); // active + downscroll + classicHitbox + hasHudStrings
  view.setUint32(8, 7, true); // hudRevision
  view.setUint32(12, 0b0010_0000_0101, true); // player mask=5, opp mask=2
  view.setInt32(16, 3, true); // countdownStage
  view.setFloat32(20, 0.75, true);
  view.setFloat64(24, 1234.5, true); // songPosMs
  view.setFloat64(32, 90000.0, true); // durationMs
  view.setFloat64(40, 2.4, true); // scrollSpeed
  view.setFloat64(48, 145.0, true); // activeBpm
  view.setFloat32(56, 1.15, true); // cameraZoom
  view.setFloat32(60, 0.0, true);
  view.setFloat32(64, 640.0, true);
  view.setFloat32(68, 360.0, true);
  view.setFloat32(72, 1280.0, true);
  view.setFloat32(76, 720.0, true);
  view.setFloat32(80, 0.82, true); // health
  view.setUint32(84, 18, true); // hits
  view.setBigInt64(88, 6300n, true); // score
  view.setUint32(96, 12, true); // combo
  view.setUint32(100, 15, true); // maxCombo
  view.setUint32(104, 1, true); // misses
  view.setUint16(108, 1, true); // entityCount
  view.setUint16(110, 1, true); // noteCount

  // Entity 60B
  let off = BINARY_GAMEPLAY_HEADER_BYTES;
  view.setFloat32(off, 960, true);
  view.setFloat32(off + 4, 420, true);
  view.setFloat32(off + 8, 300, true);
  view.setFloat32(off + 12, 340, true);
  view.setFloat32(off + 16, 2.25, true); // scale
  view.setFloat32(off + 20, 0.9, true); // opacity
  view.setUint32(off + 24, 1 | (1 << 2) | (2 << 4), true); // visible + hasFrame + player(2)
  view.setUint32(off + 28, 10, true);
  view.setUint32(off + 32, 20, true);
  view.setUint32(off + 36, 120, true);
  view.setUint32(off + 40, 220, true);
  view.setInt32(off + 44, -8, true);
  view.setInt32(off + 48, -12, true);
  view.setUint32(off + 52, 140, true);
  view.setUint32(off + 56, 250, true);
  off += BINARY_GAMEPLAY_ENTITY_BYTES;

  // Note 28B
  view.setBigUint64(off, 99n, true);
  view.setFloat64(off + 8, 2500.0, true);
  view.setFloat64(off + 16, 300.0, true);
  view.setUint32(off + 24, 2 | (1 << 2) | (1 << 3), true); // up(2) + isPlayer + headHit
  off += BINARY_GAMEPLAY_NOTE_BYTES;

  for (const chunk of [sJudg, sCd, sErr, sLog]) {
    view.setUint16(off, chunk.length, true);
    off += 2;
    buf.set(chunk, off);
    off += chunk.length;
  }

  const decoded = decodeBinaryGameplayFrame(buf);
  assert.equal(decoded.active, true);
  assert.equal(decoded.downscroll, true);
  assert.equal(decoded.controlMode, 'classic_hitbox');
  assert.equal(decoded.hasHudStrings, true);
  assert.equal(decoded.hudRevision, 7);
  assert.equal(decoded.holdingLanesMask, 5);
  assert.equal(decoded.opponentHoldingLanesMask, 2);
  assert.equal(decoded.score, 6300n);
  assert.equal(decoded.entities.length, 1);
  assert.ok(Math.abs(decoded.entities[0].scale - 2.25) < 0.001);
  assert.equal(decoded.entities[0].roleCode, 2);
  assert.equal(decoded.entities[0].hasFrame, true);
  assert.equal(decoded.entities[0].offX, -8);
  assert.equal(decoded.visibleNotes.length, 1);
  assert.equal(decoded.visibleNotes[0].direction, 2);
  assert.equal(decoded.visibleNotes[0].headHit, true);
  assert.equal(decoded.lastJudgement, '¡PERFECTO!');
  assert.equal(decoded.countdownLabel, '¡YA!');
  assert.equal(decoded.scriptLog, 'onBeatHit(4)');

  // Sin bit 5 (hasHudStrings = false) no lee strings en frames estables
  view.setUint32(4, 1, true);
  const fastFrame = decodeBinaryGameplayFrame(
    buf.subarray(0, BINARY_GAMEPLAY_HEADER_BYTES + BINARY_GAMEPLAY_ENTITY_BYTES + BINARY_GAMEPLAY_NOTE_BYTES),
  );
  assert.equal(fastFrame.hasHudStrings, false);
  assert.equal(fastFrame.lastJudgement, undefined);
});

test('clampEntityScale and sortProjectSummaries enforce valid bounds and active-first ordering', () => {
  assert.equal(clampEntityScale(0.001), 0.05);
  assert.equal(clampEntityScale(12.5), 6.0);
  assert.equal(clampEntityScale(2.75), 2.75);
  assert.equal(clampEntityScale(Number.NaN), 1.0);

  const sorted = sortProjectSummaries([
    { id: 'p1', name: 'Viejo', updatedAtMs: 1000, isActive: false },
    { id: 'p2', name: 'Reciente', updatedAtMs: 5000, isActive: false },
    { id: 'p3', name: 'Activo', updatedAtMs: 2000, isActive: true },
  ]);
  assert.deepEqual(
    sorted.map((p) => p.id),
    ['p3', 'p2', 'p1'],
  );
});

