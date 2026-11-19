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
  BINARY_GAMEPLAY_STRUM_BYTES,
  LruResourceCache,
  STREAM_RING_BYTES,
  advanceNoteSlidingWindow,
  analyzeStageEventRamLifecycle,
  autoBindSparrowPrefixes,
  buildBfaudCacheKey,
  buildFlashEffectEventParams,
  buildStageSwapEventParams,
  buildUiBlackoutEventParams,
  clampEntityScale,
  classifySafImportStorage,
  compareNaturalFrameNames,
  computeBpmSyncedIdleCadence,
  computeCameraCaptureBox,
  computeChartPlayheadFollowAction,
  computeChartSustainGridCells,
  computeCountdownState,
  computeDiskCacheBudgetSplit,
  computeReceptorLayout,
  computeSmartTextureScale,
  computeSustainTailSpan,
  computeTrimmedFramePlacement,
  computeUniformCharacterDrawBox,
  decodeBinaryGameplayFrame,
  desaturateToNeutralRgba,
  detectSpriteJsonFormat,
  estimateMergedAudioRamSavings,
  evaluateFnfHealthBar,
  evaluateHitJudgement,
  evaluateWatchdogExecution,
  fnv1a64Hex,
  mapRotatedSpriteSample,
  optimizeScriptLoops,
  packPosesIntoBoundedAtlas,
  parseAstcHeader,
  parseCameraConfigFromLua,
  parseChartJson,
  parseUiBlackoutEvent,
  planMultiProjectLruEviction,
  renderCameraLuaConfigBlock,
  resolveChartPlayheadStep,
  resolveCompiledSpriteTarget,
  resolveMobileTouchLanes,
  serializeChartJson,
  shouldDiscardInvisibleEntity,
  shouldReuseCachedAudio,
  shouldReuseCachedScript,
  shouldReuseCachedTexture,
  sortProjectSummaries,
  stripTrailingFrameDigits,
  validateAudioTrackCount,
  validateBfaudHeader,
  VANILLA_MAGIC_BYTES,
  VANILLA_HEADER_BYTES,
  VANILLA_FORMAT_VERSION,
  VANILLA_FILE_EXTENSION,
  VANILLA_FLAG_OPTIMIZED_WEBP,
  VANILLA_FLAG_ASSET_DEDUP,
  classifyVanillaEntryCodec,
  validateVanillaRelPath,
  validateVanillaHeader,
  mapAnimNameToKind,
  defaultRoleAnchor,
  normalizeStageWorldPos,
  applyCharacterPosition,
  parseModDescriptor,
  parseSpecialNoteType,
  noteSpecialTypeToCode,
  noteSpecialCodeToType,
  isHazardXNote,
  normalizeRenderBackendKind,
  matchArrowSparrowSubTexture,
  decodeBfgpExtensionFlags,
  decodeBfgpNoteKindCode,
  normalizeAndCenterAtlasFrames,
  computeStageImportDimensions,
  alignAtlasAnimationsWithRgba,
  resolveCharacterAnimWithFlipX,
  defaultStageCameraSlots,
  resolveCameraSlotTarget,
  parseChartCameraEvent,
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

test('autoBindSparrowPrefixes maps Idle, Left, Down, Up, Right and Miss animations automatically', () => {
  const bindings = autoBindSparrowPrefixes([
    'Hero_idle_dance',
    'Hero_singLEFT',
    'Hero_singDOWN',
    'Hero_singUP',
    'Hero_singRIGHT',
    'Hero_singLEFT_miss',
    'Hero_singDOWN_miss',
    'Hero_singUP_miss',
    'Hero_singRIGHT_miss',
  ]);

  assert.equal(bindings.idle, 'Hero_idle_dance');
  assert.equal(bindings.left, 'Hero_singLEFT');
  assert.equal(bindings.down, 'Hero_singDOWN');
  assert.equal(bindings.up, 'Hero_singUP');
  assert.equal(bindings.right, 'Hero_singRIGHT');
  assert.equal(bindings.missLeft, 'Hero_singLEFT_miss');
  assert.equal(bindings.missDown, 'Hero_singDOWN_miss');
  assert.equal(bindings.missUp, 'Hero_singUP_miss');
  assert.equal(bindings.missRight, 'Hero_singRIGHT_miss');
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

  // Teléfonos 32-bit / 2-3 GB RAM: fondos <= 1536px y atlas <= 2560px
  const low32Static = computeSmartTextureScale({
    origWidth: 2048,
    origHeight: 2048,
    entityWidth: 1280,
    entityHeight: 720,
    entityScale: 1.0,
    hasAtlas: false,
    tier: 'low_32bit',
  });
  assert.ok(low32Static <= 1536 / 2048 + 0.0001);

  const low32Atlas = computeSmartTextureScale({
    origWidth: 4096,
    origHeight: 4096,
    entityWidth: 500,
    entityHeight: 600,
    entityScale: 1.0,
    hasAtlas: true,
    tier: 'low_32bit',
  });
  assert.ok(low32Atlas <= 2560 / 4096 + 0.0001);
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

test('decodeBinaryGameplayFrame decodes BFGP v2 header, rotated entities, strums, dynamic items and HUD strings', () => {
  const encoder = new TextEncoder();
  const sJudg = encoder.encode('SICK!');
  const sCd = encoder.encode('¡YA!');
  const sErr = encoder.encode('');
  const sLog = encoder.encode('onBeatHit(4)');
  const dynLabel = encoder.encode('MOD TEXTO');

  const dynBytes = 32 + 2 + dynLabel.length;
  const totalBytes =
    BINARY_GAMEPLAY_HEADER_BYTES +
    BINARY_GAMEPLAY_ENTITY_BYTES +
    BINARY_GAMEPLAY_NOTE_BYTES +
    8 * BINARY_GAMEPLAY_STRUM_BYTES +
    dynBytes +
    8 +
    sJudg.length +
    sCd.length +
    sErr.length +
    sLog.length;

  const buf = new Uint8Array(totalBytes);
  const view = new DataView(buf.buffer);

  // Header 152B (BFGP v2 con flash + personajes activos)
  view.setUint32(0, BINARY_GAMEPLAY_MAGIC, true);
  view.setUint32(4, 1 | (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7), true); // + flashOverHud
  view.setUint32(8, 7, true); // hudRevision
  view.setUint32(12, 0b0010_0000_0101 | (1 << 16) | (3 << 20), true); // player=5, opp=2, rating=1(Sick), splashLane=3
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
  view.setFloat32(108, 94.5, true); // accuracyPct
  view.setFloat32(112, 0.9, true); // hudOpacity
  view.setFloat32(116, 1.05, true); // hudZoom
  view.setUint32(120, 11, true); // ratingEventId
  view.setUint32(124, 9, true); // splashEventId
  view.setFloat32(128, 0.25, true); // beatPhase
  view.setUint32(132, 0xffef4444, true); // flashColorArgb
  view.setFloat32(136, 0.65, true); // flashAlpha
  view.setInt16(140, 2, true); // activePlayerIdx
  view.setInt16(142, 1, true); // activeOpponentIdx
  view.setUint16(144, 1, true); // entityCount
  view.setUint16(146, 1, true); // noteCount
  view.setUint16(148, 8, true); // strumCount
  view.setUint16(150, 1, true); // dynCount

  // Entity 64B
  let off = BINARY_GAMEPLAY_HEADER_BYTES;
  view.setFloat32(off, 960, true);
  view.setFloat32(off + 4, 420, true);
  view.setFloat32(off + 8, 300, true);
  view.setFloat32(off + 12, 340, true);
  view.setFloat32(off + 16, 2.25, true); // scale
  view.setFloat32(off + 20, 0.9, true); // opacity
  view.setFloat32(off + 24, 15.0, true); // rotationDeg
  view.setUint32(off + 28, 1 | (1 << 2) | (2 << 4) | (1 << 6), true); // visible + hasFrame + player(2) + preloadRequested
  view.setUint32(off + 32, 10, true);
  view.setUint32(off + 36, 20, true);
  view.setUint32(off + 40, 120, true);
  view.setUint32(off + 44, 220, true);
  view.setInt32(off + 48, -8, true);
  view.setInt32(off + 52, -12, true);
  view.setUint32(off + 56, 140, true);
  view.setUint32(off + 60, 250, true);
  off += BINARY_GAMEPLAY_ENTITY_BYTES;

  // Note 28B
  view.setBigUint64(off, 99n, true);
  view.setFloat64(off + 8, 2500.0, true);
  view.setFloat64(off + 16, 300.0, true);
  view.setUint32(off + 24, 2 | (1 << 2) | (1 << 3), true); // up(2) + isPlayer + headHit
  off += BINARY_GAMEPLAY_NOTE_BYTES;

  // 8 Strums * 20B
  for (let s = 0; s < 8; s++) {
    view.setFloat32(off, s === 4 ? 22.5 : 0, true);
    view.setFloat32(off + 4, s === 4 ? -14.0 : 0, true);
    view.setFloat32(off + 8, s === 4 ? 45.0 : 0, true);
    view.setFloat32(off + 12, 1.0, true);
    view.setFloat32(off + 16, 1.0, true);
    off += BINARY_GAMEPLAY_STRUM_BYTES;
  }

  // 1 Dynamic text item
  buf[off] = 1; // isText
  buf[off + 1] = 1; // isHud
  buf[off + 2] = 1; // visible
  buf[off + 3] = 0;
  view.setFloat32(off + 4, 120, true);
  view.setFloat32(off + 8, 240, true);
  view.setFloat32(off + 12, 300, true);
  view.setFloat32(off + 16, 28, true);
  view.setFloat32(off + 20, 1.0, true);
  view.setFloat32(off + 24, 0.0, true);
  view.setUint32(off + 28, 0xffffffff, true);
  off += 32;
  view.setUint16(off, dynLabel.length, true);
  off += 2;
  buf.set(dynLabel, off);
  off += dynLabel.length;

  for (const chunk of [sJudg, sCd, sErr, sLog]) {
    view.setUint16(off, chunk.length, true);
    off += 2;
    buf.set(chunk, off);
    off += chunk.length;
  }

  const decoded = decodeBinaryGameplayFrame(buf);
  assert.equal(decoded.active, true);
  assert.equal(decoded.downscroll, true);
  assert.equal(decoded.hudVisible, true);
  assert.equal(decoded.flashOverHud, true);
  assert.equal(decoded.flashColorArgb, 0xffef4444);
  assert.ok(Math.abs(decoded.flashAlpha - 0.65) < 0.01);
  assert.equal(decoded.activePlayerIdx, 2);
  assert.equal(decoded.activeOpponentIdx, 1);
  assert.equal(decoded.controlMode, 'classic_hitbox');
  assert.equal(decoded.hasHudStrings, true);
  assert.equal(decoded.hudRevision, 7);
  assert.equal(decoded.holdingLanesMask, 5);
  assert.equal(decoded.opponentHoldingLanesMask, 2);
  assert.equal(decoded.lastRatingCode, 1);
  assert.equal(decoded.splashLane, 3);
  assert.equal(decoded.ratingEventId, 11);
  assert.equal(decoded.splashEventId, 9);
  assert.ok(Math.abs(decoded.accuracyPct - 94.5) < 0.01);
  assert.equal(decoded.score, 6300n);
  assert.equal(decoded.entities.length, 1);
  assert.ok(Math.abs(decoded.entities[0].scale - 2.25) < 0.001);
  assert.ok(Math.abs(decoded.entities[0].rotationDeg - 15.0) < 0.001);
  assert.equal(decoded.entities[0].roleCode, 2);
  assert.equal(decoded.entities[0].preloadRequested, true);
  assert.equal(decoded.entities[0].retired, false);
  assert.equal(decoded.entities[0].hasFrame, true);
  assert.equal(decoded.entities[0].offX, -8);
  assert.equal(decoded.visibleNotes.length, 1);
  assert.equal(decoded.visibleNotes[0].direction, 2);
  assert.equal(decoded.visibleNotes[0].headHit, true);
  assert.equal(decoded.strumOverrides.length, 8);
  assert.ok(Math.abs(decoded.strumOverrides[4].offsetX - 22.5) < 0.01);
  assert.ok(Math.abs(decoded.strumOverrides[4].angle - 45.0) < 0.01);
  assert.equal(decoded.dynamicItems.length, 1);
  assert.equal(decoded.dynamicItems[0].text, 'MOD TEXTO');
  assert.equal(decoded.lastJudgement, 'SICK!');
  assert.equal(decoded.countdownLabel, '¡YA!');
  assert.equal(decoded.scriptLog, 'onBeatHit(4)');

  // Sin bit 5 (hasHudStrings = false) no lee strings en frames estables
  view.setUint32(4, 1, true);
  view.setUint16(148, 0, true);
  view.setUint16(150, 0, true);
  const fastFrame = decodeBinaryGameplayFrame(
    buf.subarray(0, BINARY_GAMEPLAY_HEADER_BYTES + BINARY_GAMEPLAY_ENTITY_BYTES + BINARY_GAMEPLAY_NOTE_BYTES),
  );
  assert.equal(fastFrame.hasHudStrings, false);
  assert.equal(fastFrame.lastJudgement, undefined);
  assert.equal(fastFrame.arrowsVisible, true);
  assert.equal(fastFrame.arrowsOpacity, 1);
  assert.equal(fastFrame.screenBlackoutAlpha, 0);
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

test('estimateMergedAudioRamSavings models i16 PCM release vs 512 KB streaming ring buffer', () => {
  // 3 minutos @ 48 kHz = 8,640,000 frames (~33.0 MB por pista en i16 estéreo)
  const frames3Min = 48_000 * 180;
  const est = estimateMergedAudioRamSavings([
    { frameCount: frames3Min, muted: false },
    { frameCount: frames3Min, muted: false },
    { frameCount: frames3Min, muted: true }, // cargada en RAM del editor pero silenciada
  ]);

  assert.equal(est.mergedTracks, 2);
  assert.equal(est.streamRingBytes, STREAM_RING_BYTES);
  assert.equal(est.streamRingBytes, 512 * 1024);
  assert.equal(est.sourceRamBytes, frames3Min * 4 * 3);
  assert.ok(est.estimatedRamSavedKb > 100_000);

  const emptyEst = estimateMergedAudioRamSavings([
    { frameCount: frames3Min, muted: true },
  ]);
  assert.equal(emptyEst.mergedTracks, 0);
  assert.equal(emptyEst.streamRingBytes, 0);
});

test('computeCameraCaptureBox and camera_system.lua block stay bidirectionally synchronized', () => {
  const zoomedOut = computeCameraCaptureBox(320, 410, 0.5);
  assert.equal(zoomedOut.width, 2560);
  assert.equal(zoomedOut.height, 1440);

  const zoomedIn = computeCameraCaptureBox(980, 430, 2.0);
  assert.equal(zoomedIn.width, 640);
  assert.equal(zoomedIn.height, 360);

  const initialCfg = {
    enabled: true,
    baseX: 640,
    baseY: 360,
    baseZoom: 1.0,
    followMode: 'beats' as const,
    followIntervalBeats: 4,
    smoothSpeed: 6.0,
    panStrength: 0.72,
  };
  const luaBlock = renderCameraLuaConfigBlock({
    ...initialCfg,
    baseX: 480,
    baseY: 390,
    baseZoom: 1.25,
    followMode: 'sections',
    followIntervalBeats: 8,
  });
  const parsed = parseCameraConfigFromLua(
    `${luaBlock}\nfunction onBeatHit(beat)\n  setCameraZoom(getCameraBaseZoom() + 0.02)\nend`,
    initialCfg,
  );
  assert.equal(parsed.enabled, true);
  assert.equal(parsed.baseX, 480);
  assert.equal(parsed.baseY, 390);
  assert.equal(parsed.baseZoom, 1.25);
  assert.equal(parsed.followMode, 'sections');
  assert.equal(parsed.followIntervalBeats, 8);
});

test('computeBpmSyncedIdleCadence adapts beat interval to chart BPM and idle duration', () => {
  // 120 BPM => beatSec = 0.5s. Animación larga (0.95s > 0.675s) -> cada 2 beats
  const longIdle = computeBpmSyncedIdleCadence({
    bpm: 120,
    idleDurationSec: 0.95,
    beatIndex: 3,
  });
  assert.equal(longIdle.beatInterval, 2);
  assert.equal(longIdle.shouldTriggerBeat, false);

  const longIdleEven = computeBpmSyncedIdleCadence({
    bpm: 120,
    idleDurationSec: 0.95,
    beatIndex: 4,
  });
  assert.equal(longIdleEven.shouldTriggerBeat, true);

  // 120 BPM + animación corta (0.4s) -> cada beat
  const shortIdle = computeBpmSyncedIdleCadence({
    bpm: 120,
    idleDurationSec: 0.4,
    beatIndex: 5,
  });
  assert.equal(shortIdle.beatInterval, 1);
  assert.equal(shortIdle.shouldTriggerBeat, true);
});

test('evaluateHitJudgement and evaluateFnfHealthBar drive rating popups, Sick splashes and losing icons', () => {
  const sick = evaluateHitJudgement(28);
  assert.equal(sick.tier, 'sick');
  assert.equal(sick.ratingCode, 1);
  assert.equal(sick.triggersSplash, true);

  const good = evaluateHitJudgement(80);
  assert.equal(good.tier, 'good');
  assert.equal(good.triggersSplash, false);

  const bad = evaluateHitJudgement(130);
  assert.equal(bad.tier, 'bad');
  assert.equal(bad.triggersSplash, false);

  const lowPlayer = evaluateFnfHealthBar(0.15);
  assert.ok(Math.abs(lowPlayer.dividerRatio - 0.85) < 0.001);
  assert.equal(lowPlayer.playerLosing, true);
  assert.equal(lowPlayer.opponentLosing, false);

  const lowOpp = evaluateFnfHealthBar(0.9);
  assert.ok(Math.abs(lowOpp.dividerRatio - 0.1) < 0.001);
  assert.equal(lowOpp.playerLosing, false);
  assert.equal(lowOpp.opponentLosing, true);

  const modLoop = optimizeScriptLoops(
    'lua',
    `function onUpdate(dt)\n  for i = 0, 1000 do setStrumProp(i % 8, "y", 10) end\nend`,
  );
  assert.equal(modLoop.discardedLoops, 0);
  assert.equal(modLoop.neutralizedLoops, 1);
  assert.ok(modLoop.optimizedSource.includes('setStrumProp'));
});

test('incremental cache fingerprints reuse valid texture/audio/script hits and invalidate on config or file changes', () => {
  const hashA = fnv1a64Hex('sprite_bytes_v1');
  const hashB = fnv1a64Hex('sprite_bytes_v2');
  assert.equal(hashA.length, 16);
  assert.notEqual(hashA, hashB);

  const texPrev = {
    entityId: 'hero',
    tier: 'balanced' as const,
    file: { fileLenBytes: 4096, modifiedEpochMs: 1700000, contentHash: hashA },
    hasAtlas: true,
    entityWidth: 320,
    entityHeight: 420,
    entityScale: 1.0,
  };
  assert.equal(shouldReuseCachedTexture(texPrev, { ...texPrev }, true), true);
  assert.equal(shouldReuseCachedTexture(texPrev, { ...texPrev }, false), false);
  assert.equal(shouldReuseCachedTexture(texPrev, { ...texPrev, tier: 'low_32bit' }, true), false);
  assert.equal(
    shouldReuseCachedTexture(
      texPrev,
      { ...texPrev, file: { ...texPrev.file, contentHash: hashB } },
      true,
    ),
    false,
  );

  const audioPrev = {
    sampleRate: 48000,
    slots: [
      {
        slotIndex: 0,
        file: { fileLenBytes: 100000, modifiedEpochMs: 1700000, contentHash: hashA },
        volume: 1.0,
        muted: false,
      },
    ],
  };
  assert.equal(shouldReuseCachedAudio(audioPrev, { ...audioPrev }, true), true);
  assert.equal(
    shouldReuseCachedAudio(
      audioPrev,
      { ...audioPrev, slots: [{ ...audioPrev.slots[0], volume: 0.65 }] },
      true,
    ),
    false,
  );

  const scriptPrev = {
    scriptId: 'mod_cam',
    language: 'lua' as const,
    optimizedSourceHash: hashA,
    artifactExists: true,
  };
  assert.equal(
    shouldReuseCachedScript(
      scriptPrev,
      { scriptId: 'mod_cam', language: 'lua', optimizedSourceHash: hashA },
      true,
    ),
    true,
  );
  assert.equal(
    shouldReuseCachedScript(
      scriptPrev,
      { scriptId: 'mod_cam', language: 'lua', optimizedSourceHash: hashB },
      true,
    ),
    false,
  );
});

test('resolveCompiledSpriteTarget converts non-WebP or downscaled sprites to lossless .webp cache', () => {
  const passthrough = resolveCompiledSpriteTarget({
    entityId: 'bg',
    sourcePath: '/cache/bg.webp',
    cacheDir: '/cache/compiled',
    scaleFactor: 1.0,
  });
  assert.equal(passthrough.mode, 'passthrough_webp');
  assert.equal(passthrough.outputPath, '/cache/bg.webp');

  const pngToWebp = resolveCompiledSpriteTarget({
    entityId: 'hero',
    sourcePath: '/imports/hero.png',
    cacheDir: '/cache/compiled/',
    scaleFactor: 1.0,
  });
  assert.equal(pngToWebp.mode, 'encode_lossless_webp');
  assert.equal(pngToWebp.outputPath, '/cache/compiled/hero.webp');

  const scaledWebp = resolveCompiledSpriteTarget({
    entityId: 'stage_fx',
    sourcePath: '/imports/stage_fx.webp',
    cacheDir: '/cache/compiled',
    scaleFactor: 0.5,
  });
  assert.equal(scaledWebp.mode, 'encode_lossless_webp');
  assert.equal(scaledWebp.outputPath, '/cache/compiled/stage_fx.webp');
});

test('evaluateWatchdogExecution enforces 250ms init / 25ms frame budgets and quarantines after 3 trips', () => {
  const initOk = evaluateWatchdogExecution({ phase: 'init', elapsedMs: 120, previousTrips: 0 });
  assert.equal(initOk.timedOut, false);
  assert.equal(initOk.quarantined, false);

  const trip1 = evaluateWatchdogExecution({ phase: 'frame', elapsedMs: 32, previousTrips: 0 });
  assert.equal(trip1.timedOut, true);
  assert.equal(trip1.tripCount, 1);
  assert.equal(trip1.quarantined, false);

  const trip3 = evaluateWatchdogExecution({ phase: 'frame', elapsedMs: 40, previousTrips: 2 });
  assert.equal(trip3.timedOut, true);
  assert.equal(trip3.tripCount, 3);
  assert.equal(trip3.quarantined, true);

  const skipped = evaluateWatchdogExecution({ phase: 'frame', elapsedMs: 1, previousTrips: 3 });
  assert.equal(skipped.skippedDueToQuarantine, true);
  assert.equal(skipped.quarantined, true);
});

test('computeChartSustainGridCells propagates continuous head->body->tail across section boundaries', () => {
  // 120 BPM => 125ms/step, section 0 has 16 steps (0..2000ms), section 1 starts at 2000ms
  const cells = computeChartSustainGridCells({
    sections: [
      { bpm: 120, lengthInSteps: 16, startMs: 0 },
      { bpm: 120, lengthInSteps: 16, startMs: 2000 },
    ],
    notes: [
      // Nota en paso 14 (1750ms) con sustain de 4 pasos (500ms -> termina en Sec 1 paso 2)
      { timeMs: 1750, direction: 1, isPlayer: true, sustainMs: 500 },
    ],
  });

  assert.equal(cells.length, 5);
  assert.deepEqual(
    cells.map((c) => [c.sectionIndex, c.stepInSection, c.col, c.kind]),
    [
      [0, 14, 5, 'head'],
      [0, 15, 5, 'body'],
      [1, 0, 5, 'body'],
      [1, 1, 5, 'body'],
      [1, 2, 5, 'tail'],
    ],
  );
});

test('analyzeStageEventRamLifecycle schedules 4s lookahead preloads, detects reused sprites and formats flash/swap cues', () => {
  const swap1 = buildStageSwapEventParams({
    showIds: ['stage_phase2', 'bf_dark'],
    hideIds: ['stage_phase1', 'bf_main'],
    playerId: 'bf_dark',
    disposeOld: true,
  });
  const swapReturn = buildStageSwapEventParams({
    showIds: ['stage_phase1'],
    hideIds: ['stage_phase2'],
    playerId: 'bf_main',
    disposeOld: false,
  });
  const flash = buildFlashEffectEventParams({
    colorHex: '#FF3355',
    startMs: 10000,
    endMs: 11000,
    alpha: 0.9,
    overHud: true,
  });

  const report = analyzeStageEventRamLifecycle([
    { timeMs: 10000, name: 'Flash Effect', ...flash },
    { timeMs: 10000, name: 'Stage Swap', ...swap1 },
    { timeMs: 25000, name: 'Stage Swap', ...swapReturn },
  ]);

  assert.ok(report.preservedIds.includes('stage_phase2'));
  assert.ok(report.preservedIds.includes('bf_dark'));
  assert.ok(report.reusedLaterIds.includes('stage_phase1'));
  assert.ok(report.reusedLaterIds.includes('bf_main'));
  assert.equal(
    report.preloadWindows.find((w) => w.entityId === 'stage_phase2')?.preloadAtMs,
    6000,
  );
  assert.ok(flash.value2.includes('dur=1.000;endMs=11000.00;alpha=0.90;hud=1'));

  // El optimizador de bucles conserva las nuevas APIs sin descartarlas
  const loopRes = optimizeScriptLoops(
    'lua',
    'for i = 1, 5000 do\n  flashEffect("#FFFFFF", 0.5, 0.8, true)\n  swapEntity("a", "b", true)\nend',
  );
  assert.equal(loopRes.discardedLoops, 0);
  assert.equal(loopRes.neutralizedLoops, 1);
  assert.ok(loopRes.optimizedSource.includes('flashEffect'));
});

test('resolveChartPlayheadStep tracks sub-step position across BPM changes and follow action respects Seguir/Libre toggle', () => {
  // Sec 0: 120 BPM (125ms/paso, 16 pasos -> 0..2000ms)
  // Sec 1: 150 BPM (100ms/paso, 16 pasos -> 2000..3600ms)
  const sections = [
    { bpm: 120, lengthInSteps: 16, startMs: 0 },
    { bpm: 150, lengthInSteps: 16, startMs: 2000 },
  ];

  // Mitad del paso #4 en Sec 0: 4 * 125 + 62.5 = 562.5 ms
  const m0 = resolveChartPlayheadStep({ sections, songPosMs: 562.5 });
  assert.ok(m0);
  assert.equal(m0.sectionIndex, 0);
  assert.equal(m0.stepInSection, 4);
  assert.ok(Math.abs(m0.stepFraction - 0.5) < 0.001);

  // Sec 1 paso #2 + 25% (2000 + 2 * 100 + 25 = 2225 ms)
  const m1 = resolveChartPlayheadStep({ sections, songPosMs: 2225 });
  assert.ok(m1);
  assert.equal(m1.sectionIndex, 1);
  assert.equal(m1.stepInSection, 2);
  assert.equal(m1.globalStep, 18);
  assert.ok(Math.abs(m1.stepFraction - 0.25) < 0.001);

  // Auto-seguimiento activo vs pausado ("Libre")
  const freeMode = computeChartPlayheadFollowAction({
    followThread: false,
    threadVisible: true,
    threadY: 600,
    viewportStart: 0,
    viewportEnd: 500,
  });
  assert.equal(freeMode.action, 'none');

  const scrollFollow = computeChartPlayheadFollowAction({
    followThread: true,
    threadVisible: true,
    threadY: 260,
    viewportStart: 0,
    viewportEnd: 500,
  });
  assert.equal(scrollFollow.action, 'scroll_by');
  assert.equal(scrollFollow.deltaY, 60); // anchorY = 500 * 0.40 = 200

  const offscreenSnap = computeChartPlayheadFollowAction({
    followThread: true,
    threadVisible: false,
    threadY: 0,
    viewportStart: 0,
    viewportEnd: 500,
  });
  assert.equal(offscreenSnap.action, 'snap_to_row');
});

test('classifySafImportStorage isolates transient imports to cacheDir staging and limits persistent binary storage', () => {
  for (const transient of ['chart', 'sparrow_xml', 'script', 'arrow_svg', 'health_bar_svg'] as const) {
    const res = classifySafImportStorage(transient);
    assert.equal(res.stagingDir, 'cacheDir/import_staging');
    assert.equal(res.policy, 'staging_then_embed');
    assert.equal(res.persistsBinaryInFilesDir, false);
    assert.equal(res.deleteStagedAfterImport, true);
  }

  for (const persistent of ['audio_track', 'sprite_png', 'character_icon'] as const) {
    const res = classifySafImportStorage(persistent);
    assert.equal(res.stagingDir, 'cacheDir/import_staging');
    assert.equal(res.policy, 'staging_then_adopt');
    assert.equal(res.persistsBinaryInFilesDir, true);
    assert.equal(res.deleteStagedAfterImport, true);
  }
});

test('computeDiskCacheBudgetSplit, planMultiProjectLruEviction and .bfaud audio cache enforce quota and protect active mod', () => {
  const clampedLow = computeDiskCacheBudgetSplit(10 * 1024 * 1024);
  assert.equal(clampedLow.totalBytes, 48 * 1024 * 1024);
  const clampedHigh = computeDiskCacheBudgetSplit(1024 * 1024 * 1024);
  assert.equal(clampedHigh.totalBytes, 384 * 1024 * 1024);

  const eviction = planMultiProjectLruEviction(
    [
      { projectId: 'mod_a', sizeBytes: 40_000_000, lastUsedMs: 1000 },
      { projectId: 'mod_active', sizeBytes: 45_000_000, lastUsedMs: 500 }, // más antiguo pero activo -> protegido
      { projectId: 'mod_b', sizeBytes: 35_000_000, lastUsedMs: 3000 },
    ],
    85_000_000,
    'mod_active',
  );
  assert.deepEqual(eviction.evictedIds, ['mod_a']);
  assert.deepEqual(eviction.keptIds, ['mod_active', 'mod_b']);
  assert.equal(eviction.bytesAfter, 80_000_000);

  const key1 = buildBfaudCacheKey({
    sizeBytes: 3_500_000,
    modifiedMs: 1700000100,
    sampleHash: 'a1b2c3d4',
    targetSampleRate: 48000,
  });
  const key2 = buildBfaudCacheKey({
    sizeBytes: 3_500_000,
    modifiedMs: 1700000100,
    sampleHash: 'a1b2c3d4',
    targetSampleRate: 44100,
  });
  assert.notEqual(key1, key2);
  assert.ok(key1.endsWith('_48000.bfaud'));

  const header = new Uint8Array(24);
  const view = new DataView(header.buffer);
  header.set([0x42, 0x46, 0x41, 0x55], 0); // "BFAU"
  view.setUint32(4, 1, true);
  view.setUint32(8, 48000, true);
  view.setUint32(12, 64, true);
  view.setBigUint64(16, 4096n, true);
  const info = validateBfaudHeader(header);
  assert.equal(info.sampleRate, 48000);
  assert.equal(info.peakCount, 64);
  assert.equal(info.expectedFileBytes, 24 + 64 * 4 + 4096 * 2);
});

test('Vanilla .mod container validates VNLA header, Zstd/Raw codec selection and safe relative paths', () => {
  assert.equal(VANILLA_FILE_EXTENSION, 'mod');

  // Clasificación híbrida: texto/WAV/ASTC a Zstd; media ya comprimida a Raw
  assert.deepEqual(classifyVanillaEntryCodec('project.json'), { codec: 'zstd', level: 12 });
  assert.deepEqual(classifyVanillaEntryCodec('scripts/stage_events.lua'), { codec: 'zstd', level: 12 });
  assert.deepEqual(classifyVanillaEntryCodec('assets/audio/inst.wav'), { codec: 'zstd', level: 9 });
  assert.deepEqual(classifyVanillaEntryCodec('assets/sprites/boss.astc'), { codec: 'zstd', level: 9 });
  assert.deepEqual(classifyVanillaEntryCodec('assets/audio/voices.ogg'), { codec: 'raw', level: 0 });
  assert.deepEqual(classifyVanillaEntryCodec('assets/sprites/bf.webp'), { codec: 'raw', level: 0 });

  // Seguridad contra path traversal
  assert.equal(validateVanillaRelPath('assets/sprites/bf.webp'), 'assets/sprites/bf.webp');
  assert.throws(() => validateVanillaRelPath('../escape.json'));
  assert.throws(() => validateVanillaRelPath('/etc/passwd'));
  assert.throws(() => validateVanillaRelPath('C:\\mods\\bad.png'));
  assert.throws(() => validateVanillaRelPath('assets//empty.png'));

  // Cabecera binaria VNLA (24 bytes LE)
  const buf = new Uint8Array(VANILLA_HEADER_BYTES);
  buf.set(VANILLA_MAGIC_BYTES, 0);
  const view = new DataView(buf.buffer);
  view.setUint16(4, VANILLA_FORMAT_VERSION, true);
  view.setUint16(6, VANILLA_FLAG_OPTIMIZED_WEBP | VANILLA_FLAG_ASSET_DEDUP, true);
  view.setUint32(8, 1420, true);
  view.setUint32(12, 8192, true);
  view.setBigUint64(16, 0x0123456789abcdefn, true);

  const parsed = validateVanillaHeader(buf);
  assert.equal(parsed.version, 1);
  assert.equal(parsed.optimizedWebp, true);
  assert.equal(parsed.assetDedup, true);
  assert.equal(parsed.manifestCompressedBytes, 1420);
  assert.equal(parsed.manifestUncompressedBytes, 8192);
  assert.equal(parsed.manifestFnv1a64Hex, '0123456789abcdef');
  assert.equal(parsed.dataSectionOffset, VANILLA_HEADER_BYTES + 1420);

  // Rechazar magic inválido o manifiesto sobredimensionado
  const badMagic = new Uint8Array(buf);
  badMagic[0] = 0x50; // "PK.."
  assert.throws(() => validateVanillaHeader(badMagic));
  assert.throws(() => validateVanillaHeader(new Uint8Array(12)));

  // Staging SAF también clasifica mod_pkg en cacheDir/import_staging con limpieza inmediata
  const modSaf = classifySafImportStorage('mod_pkg');
  assert.equal(modSaf.stagingDir, 'cacheDir/import_staging');
  assert.equal(modSaf.deleteStagedAfterImport, true);
});

test('parseModDescriptor parses Psych and V-Slice character JSON with exact offset signs and relative placement', () => {
  const psychChar = JSON.stringify({
    animations: [
      { anim: 'idle', name: 'Hero Idle', fps: 24, loop: false, offsets: [0, 0] },
      { anim: 'singLEFT', name: 'Hero Left', fps: 24, loop: false, offsets: [12, -8] },
      { anim: 'singUPmiss', name: 'Hero MissUp', fps: 24, loop: false, offsets: [-20, 30] },
    ],
    flip_x: true,
    scale: 1.15,
    position: [40, 200],
    camera_position: [15, -40],
  });

  const parsedPsych = parseModDescriptor(psychChar);
  assert.equal(parsedPsych.kind, 'character');
  assert.equal(parsedPsych.isRelativePosition, true);
  assert.deepEqual(parsedPsych.position, [40, 200]);
  assert.deepEqual(parsedPsych.cameraPosition, [15, -40]);
  assert.equal(parsedPsych.scale, 1.15);
  assert.equal(parsedPsych.flipX, true);
  assert.equal(parsedPsych.animations.length, 3);
  assert.equal(parsedPsych.animations[1].animKey, 'left');
  assert.equal(parsedPsych.animations[1].prefix, 'Hero Left');
  // Psych resta offsets -> en BeatForge (+X/+Y) se niegan
  assert.deepEqual(parsedPsych.animations[1].offset, [-12, 8]);
  assert.equal(parsedPsych.animations[2].animKey, 'missUp');
  assert.deepEqual(parsedPsych.animations[2].offset, [20, -30]);

  const relApplied = applyCharacterPosition(defaultRoleAnchor('player'), [40, 200], true, 'player');
  assert.deepEqual(relApplied, [980 + 40 * 0.55, 430 + 200 * 0.25]);

  // V-Slice conserva offsets con signo positivo y coordenadas absolutas
  const vsliceChar = JSON.stringify({
    assetPath: 'characters/hero',
    startingAnimation: 'idle',
    animations: [
      { name: 'idle', prefix: 'HeroIdle', fps: 24, looped: true, offsets: [5, 10] },
      { name: 'singDOWN', prefix: 'HeroDown', fps: 24, looped: false, offsets: [-6, 14] },
    ],
    offsets: [920, 440],
  });
  const parsedVslice = parseModDescriptor(vsliceChar);
  assert.equal(parsedVslice.kind, 'character');
  assert.equal(parsedVslice.isRelativePosition, false);
  assert.deepEqual(parsedVslice.animations[1].animKey, 'down');
  assert.deepEqual(parsedVslice.animations[1].offset, [-6, 14]);
});

test('parseModDescriptor parses Codename <character> and <stage> XML descriptors accurately', () => {
  const charXml = `
  <character x="20" y="100" camx="10" camy="-25" flipX="false" scale="0.95">
    <anim name="idle" anim="Rival idle" fps="24" loop="true" x="0" y="0"/>
    <anim name="singRIGHT" anim="Rival right" fps="24" loop="false" x="-14" y="22"/>
  </character>`;
  const parsedChar = parseModDescriptor(charXml);
  assert.equal(parsedChar.kind, 'character');
  assert.deepEqual(parsedChar.position, [20, 100]);
  assert.deepEqual(parsedChar.cameraPosition, [10, -25]);
  assert.equal(parsedChar.scale, 0.95);
  assert.equal(parsedChar.flipX, false);
  assert.equal(parsedChar.animations[0].animKey, 'idle');
  assert.equal(parsedChar.animations[0].looped, true);
  assert.equal(parsedChar.animations[1].animKey, 'right');
  assert.deepEqual(parsedChar.animations[1].offset, [14, -22]);

  const stageXml = `
  <stage zoom="0.92" startCamPosX="650" startCamPosY="340">
    <sprite name="sky" sprite="stages/sky" x="0" y="0" scale="1.2" scrollx="0.5" scrolly="0.5"/>
    <dad x="120" y="180" camxoffset="40" camyoffset="-20"/>
    <girlfriend x="480" y="140"/>
    <boyfriend x="820" y="200" camxoffset="-30" camyoffset="-15"/>
  </stage>`;
  const parsedStage = parseModDescriptor(stageXml);
  assert.equal(parsedStage.kind, 'stage');
  assert.ok(Math.abs((parsedStage.defaultZoom ?? 0) - 0.92) < 0.001);
  assert.deepEqual(parsedStage.cameraStart, [650, 340]);
  assert.deepEqual(parsedStage.playerPos, [820, 200]);
  assert.deepEqual(parsedStage.opponentPos, [120, 180]);
  assert.deepEqual(parsedStage.gfPos, [480, 140]);
  assert.equal(parsedStage.props.length, 1);
  assert.equal(parsedStage.props[0].name, 'sky');
  assert.equal(parsedStage.props[0].scrollFactor, 0.5);
});

test('parseModDescriptor parses Psych/V-Slice stage JSON and normalizes world coordinates to 1280x720', () => {
  const psychStage = JSON.stringify({
    defaultZoom: 0.9,
    boyfriend: [820, 220],
    girlfriend: [450, 140],
    opponent: [110, 220],
    hide_girlfriend: true,
  });
  const st1 = parseModDescriptor(psychStage);
  assert.equal(st1.kind, 'stage');
  assert.equal(st1.isBottomCenterCoords, false);
  assert.equal(st1.hideGf, true);
  assert.deepEqual(normalizeStageWorldPos(st1.playerPos!, 'player', st1.isBottomCenterCoords), [960, 400]);

  const vsliceStage = JSON.stringify({
    cameraZoom: 1.1,
    props: [
      {
        name: 'stageBack',
        assetPath: 'stages/main/back',
        position: [0, 0],
        scale: [1.0, 1.0],
        scroll: [0.9, 0.9],
        zIndex: 10,
      },
    ],
    characters: {
      bf: { zIndex: 300, position: [990, 880], cameraOffsets: [-100, -100] },
      dad: { zIndex: 200, position: [335, 880], cameraOffsets: [150, -100] },
    },
  });
  const st2 = parseModDescriptor(vsliceStage);
  assert.equal(st2.kind, 'stage');
  assert.equal(st2.isBottomCenterCoords, true);
  assert.equal(st2.playerZ, 300);
  assert.equal(st2.props.length, 1);
  const bfWorld = normalizeStageWorldPos(st2.playerPos!, 'player', st2.isBottomCenterCoords);
  assert.equal(bfWorld[0], 990);
  assert.ok(Math.abs(bfWorld[1] - 880 * 0.56) < 0.01);
});

test('parseModDescriptor rejects empty or unrecognized mod descriptor payloads cleanly', () => {
  assert.equal(mapAnimNameToKind('singLEFT-alt'), 'left');
  assert.equal(mapAnimNameToKind('singRIGHTmiss'), 'missRight');
  assert.throws(() => parseModDescriptor('   '));
  assert.throws(() => parseModDescriptor('<TextureAtlas imagePath="sheet.png"></TextureAtlas>'));
  assert.throws(() => parseModDescriptor(JSON.stringify({ unrelated: true })));
});

test('decodeBinaryGameplayFrame decodes arrowsVisible, quantized arrowsOpacity and screenBlackoutAlpha', () => {
  const buf = new Uint8Array(BINARY_GAMEPLAY_HEADER_BYTES);
  const view = new DataView(buf.buffer);
  view.setUint32(0, BINARY_GAMEPLAY_MAGIC, true);
  // bit 0 (active=1), bit 8 (arrowsHidden=1), bits 16..23 (blackout=204 ~ 0.8), bits 24..31 (arrowsFadeOut=153 -> opacity ~ 0.4)
  const flags = (1 | (1 << 8) | (204 << 16) | (153 << 24)) >>> 0;
  view.setUint32(4, flags, true);

  const decoded = decodeBinaryGameplayFrame(buf);
  assert.equal(decoded.active, true);
  assert.equal(decoded.hudVisible, false);
  assert.equal(decoded.arrowsVisible, false);
  assert.ok(Math.abs(decoded.screenBlackoutAlpha - 204 / 255) < 0.005);
  assert.ok(Math.abs(decoded.arrowsOpacity - (1 - 153 / 255)) < 0.005);
});

test('UI Blackout helpers support intro blackout keeping 3-2-1-GO visible and external chart.json events', () => {
  const introBuilt = buildUiBlackoutEventParams({
    action: 'intro',
    blackAlpha: 1,
    hideHud: true,
    hideArrows: true,
    durationSec: 0.85,
    revealDelayMs: 500,
    revealOnSongStart: true,
  });
  assert.equal(introBuilt.name, 'UI Blackout');
  const introParsed = parseUiBlackoutEvent({ timeMs: 0, ...introBuilt });
  assert.equal(introParsed.isUiBlackoutEvent, true);
  assert.equal(introParsed.isIntroBlackout, true);
  assert.equal(introParsed.blackAlpha, 1);
  assert.equal(introParsed.hudAlpha, 0);
  assert.equal(introParsed.arrowsAlpha, 0);
  assert.equal(introParsed.revealDelayMs, 500);
  assert.equal(introParsed.revealOnSongStart, true);

  const extScreenFade = parseUiBlackoutEvent({
    timeMs: 12000,
    name: 'Screen Fade',
    value1: '0.75',
    value2: '1.25',
  });
  assert.equal(extScreenFade.isUiBlackoutEvent, true);
  assert.equal(extScreenFade.isIntroBlackout, false);
  assert.equal(extScreenFade.blackAlpha, 0.75);
  assert.equal(extScreenFade.durationSec, 1.25);

  const extHideUi = parseUiBlackoutEvent({
    timeMs: 32000,
    name: 'Hide UI',
    value1: '',
    value2: '',
  });
  assert.equal(extHideUi.isUiBlackoutEvent, true);
  assert.equal(extHideUi.action, 'hide');
  assert.equal(extHideUi.hudAlpha, 0);
  assert.equal(extHideUi.arrowsAlpha, 0);
  assert.equal(extHideUi.blackAlpha, -1);

  const luaOpt = optimizeScriptLoops(
    'lua',
    `function onSongStart()
       for i = 1, 2 do
         tweenScreenBlackout('intro', 0, 0.8, 'sineOut')
         tweenArrowsOpacity('arrows', 1, 0.8, 'sineOut')
       end
     end`,
  );
  assert.equal(luaOpt.discardedLoops, 0);
  assert.ok(luaOpt.optimizedSource.includes('tweenScreenBlackout'));
});

test('parseSpecialNoteType and note codes normalize hazard X, dodge, alt and no_anim notes', () => {
  assert.equal(parseSpecialNoteType(''), 'default');
  assert.equal(parseSpecialNoteType('Hurt Note'), 'hurt');
  assert.equal(parseSpecialNoteType('mine'), 'hurt');
  assert.equal(parseSpecialNoteType('Kill_Mine'), 'instakill');
  assert.equal(parseSpecialNoteType('Dodge Note'), 'dodge');
  assert.equal(parseSpecialNoteType('Alt Animation'), 'alt');
  assert.equal(parseSpecialNoteType('No Animation'), 'no_anim');

  assert.equal(isHazardXNote('hurt'), true);
  assert.equal(isHazardXNote('instakill'), true);
  assert.equal(isHazardXNote('dodge'), false);
  assert.equal(isHazardXNote('default'), false);

  for (const kind of ['default', 'hurt', 'instakill', 'dodge', 'alt', 'no_anim'] as const) {
    const code = noteSpecialTypeToCode(kind);
    assert.equal(noteSpecialCodeToType(code), kind);
  }
});

test('matchArrowSparrowSubTexture classifies standard mod NOTE_assets.xml entries and skips holds/confirm', () => {
  assert.deepEqual(matchArrowSparrowSubTexture('purple0000'), { kind: 'arrow', direction: 'left' });
  assert.deepEqual(matchArrowSparrowSubTexture('blue0000'), { kind: 'arrow', direction: 'down' });
  assert.deepEqual(matchArrowSparrowSubTexture('green0000'), { kind: 'arrow', direction: 'up' });
  assert.deepEqual(matchArrowSparrowSubTexture('red0000'), { kind: 'arrow', direction: 'right' });
  assert.deepEqual(matchArrowSparrowSubTexture('arrowLEFT0000'), { kind: 'receptor', direction: 'left' });
  assert.deepEqual(matchArrowSparrowSubTexture('hurt0000'), { kind: 'hurt' });
  assert.equal(matchArrowSparrowSubTexture('left confirm0000'), null);
  assert.equal(matchArrowSparrowSubTexture('pruple end hold0000'), null);
  assert.equal(matchArrowSparrowSubTexture('blue hold piece0000'), null);
});

test('decodeBfgpExtensionFlags, noteKindCode and normalizeRenderBackendKind preserve BFGP v2 and GPU contract', () => {
  const flags = (1 | (1 << 9) | (2 << 10) | (1 << 12) | (1 << 13)) >>> 0;
  const ext = decodeBfgpExtensionFlags(flags);
  assert.equal(ext.gameOver, true);
  assert.equal(ext.gameOverPhase, 2);
  assert.equal(ext.botplay, true);
  assert.equal(ext.practiceMode, true);

  const noteFlags = (3 & 0x3) | (1 << 2) | (2 << 4);
  assert.equal(decodeBfgpNoteKindCode(noteFlags), 2);
  assert.equal(noteSpecialCodeToType(decodeBfgpNoteKindCode(noteFlags)), 'instakill');

  assert.equal(normalizeRenderBackendKind('gles2'), 'gles2');
  assert.equal(normalizeRenderBackendKind('OpenGL_ES3'), 'gles3');
  assert.equal(normalizeRenderBackendKind('vulkan'), 'vulkan');
  assert.equal(normalizeRenderBackendKind('unknown'), 'canvas2d');
});

test('normalizeAndCenterAtlasFrames centers misaligned poses, aligns untrimmed frames, and is idempotent', () => {
  const input = {
    idle: [
      { width: 100, height: 150, frameX: -10, frameY: -6, frameWidth: 180, frameHeight: 200 },
      { width: 104, height: 154, frameX: -8, frameY: -2, frameWidth: 180, frameHeight: 200 },
    ],
    singLEFT: [
      { width: 160, height: 170, frameX: -90, frameY: -35, frameWidth: 320, frameHeight: 210 },
    ],
    singUP: [
      // Untrimmed JSON/Animate frames without frameX/frameY
      { width: 120, height: 210, frameX: 0, frameY: 0, frameWidth: 120, frameHeight: 210 },
    ],
  };

  const first = normalizeAndCenterAtlasFrames(input);
  assert.equal(first.unifiedWidth, 160);
  assert.equal(first.unifiedHeight, 210);

  // Idle span = 104x154 (minY = 2) -> padX = (160 - 104)/2 = 28, padY = 210 - 154 = 56
  // idle[0] ry = 6 - 2 = 4 -> frameY = -60; idle[1] ry = 2 - 2 = 0 -> frameY = -56
  assert.equal(first.animations.idle[0].frameWidth, 160);
  assert.equal(first.animations.idle[0].frameHeight, 210);
  assert.equal(first.animations.idle[0].frameX, -30);
  assert.equal(first.animations.idle[0].frameY, -60);
  assert.equal(first.animations.idle[1].frameX, -28);
  assert.equal(first.animations.idle[1].frameY, -56);

  // singLEFT was shifted far right (-90, -35), now centered at floor (0, -40)
  assert.equal(first.animations.singLEFT[0].frameX, 0);
  assert.equal(first.animations.singLEFT[0].frameY, -40);

  // Untrimmed singUP centered horizontally: padX = (160 - 120)/2 = 20, floor aligned = 0
  assert.equal(first.animations.singUP[0].frameX, -20);
  assert.equal(first.animations.singUP[0].frameY, 0);

  // Idempotent on second pass
  const second = normalizeAndCenterAtlasFrames(first.animations);
  assert.deepEqual(second, first);
});

test('computeStageImportDimensions prevents full-sheet squashing and scales small/large imports cleanly', () => {
  // Character imported with XML/JSON keeps ~380px idle height even with wide sing pose
  const charFromAtlas = computeStageImportDimensions({
    role: 'player',
    width: 480,
    height: 380,
    isAtlas: true,
    idleHeight: 360,
  });
  assert.ok(charFromAtlas.height >= 360 && charFromAtlas.height <= 420);
  assert.ok(charFromAtlas.width >= 480);

  // Small pixel sprite scales up comfortably instead of staying tiny
  const pixelProp = computeStageImportDimensions({
    role: 'decoration',
    width: 64,
    height: 64,
    isAtlas: false,
  });
  assert.ok(pixelProp.width >= 300 && pixelProp.height >= 300);

  // Background covers 1280x720 stage
  const bg = computeStageImportDimensions({
    role: 'background',
    width: 960,
    height: 540,
    isAtlas: false,
  });
  assert.ok(bg.width >= 1280 && bg.height >= 720);
});

test('alignAtlasAnimationsWithRgba aligns asymmetric poses and rotated frames to foot anchor idempotently', () => {
  const imgW = 500;
  const imgH = 200;
  const rgba = new Uint8Array(imgW * imgH * 4);
  const paintRect = (rx: number, ry: number, rw: number, rh: number) => {
    for (let py = ry; py < ry + rh; py += 1) {
      for (let px = rx; px < rx + rw; px += 1) {
        const idx = (py * imgW + px) * 4;
        rgba[idx] = 255;
        rgba[idx + 1] = 255;
        rgba[idx + 2] = 255;
        rgba[idx + 3] = 255;
      }
    }
  };

  // idle (0..80 x 0..120): botas en lx=25..55 (centro=40), suelo ly=120
  paintRect(25, 10, 30, 110);
  // singRIGHT (100..260 x 0..150): botas en lx=20..50 (centro=35), suelo ly=110 + brazo derecho + 40px vacío abajo
  paintRect(120, 10, 30, 100);
  paintRect(150, 20, 105, 50);
  // singLEFT rotado (300..420 x 0..140 -> vis 140x120): botas visual lx=95..125 (centro=110), ly=10..115 (suelo=115)
  for (let ly = 10; ly < 115; ly += 1) {
    for (let lx = 95; lx < 125; lx += 1) {
      const tx = 300 + (120 - 1 - ly);
      const ty = lx;
      const idx = (ty * imgW + tx) * 4;
      rgba[idx] = 255;
      rgba[idx + 1] = 255;
      rgba[idx + 2] = 255;
      rgba[idx + 3] = 255;
    }
  }
  for (let ly = 20; ly < 65; ly += 1) {
    for (let lx = 0; lx < 95; lx += 1) {
      const tx = 300 + (120 - 1 - ly);
      const ty = lx;
      const idx = (ty * imgW + tx) * 4;
      rgba[idx] = 255;
      rgba[idx + 1] = 255;
      rgba[idx + 2] = 255;
      rgba[idx + 3] = 255;
    }
  }

  const input = {
    idle: [{ x: 0, y: 0, width: 80, height: 120, frameX: 0, frameY: 0, frameWidth: 80, frameHeight: 120 }],
    right: [{ x: 100, y: 0, width: 160, height: 150, frameX: 0, frameY: 0, frameWidth: 160, frameHeight: 150 }],
    left: [{ x: 300, y: 0, width: 120, height: 140, frameX: 0, frameY: 0, frameWidth: 140, frameHeight: 120, rotated: true }],
  };

  const first = alignAtlasAnimationsWithRgba(input, imgW, imgH, rgba);
  const center = first.unifiedWidth * 0.5;
  const fIdle = first.animations.idle[0];
  const fRight = first.animations.right[0];
  const fLeft = first.animations.left[0];

  assert.equal(fIdle.frameWidth, first.unifiedWidth);
  assert.equal(fRight.frameWidth, first.unifiedWidth);
  assert.equal(fLeft.frameWidth, first.unifiedWidth);

  const floorIdle = -fIdle.frameY + 120;
  const floorRight = -fRight.frameY + 110;
  const floorLeft = -fLeft.frameY + 115;
  assert.ok(Math.abs(floorIdle - floorRight) <= 1);
  assert.ok(Math.abs(floorIdle - floorLeft) <= 1);

  assert.ok(Math.abs(-fIdle.frameX + 40 - center) <= 1);
  assert.ok(Math.abs(-fRight.frameX + 35 - center) <= 1);
  assert.ok(Math.abs(-fLeft.frameX + 110 - center) <= 1);

  const second = alignAtlasAnimationsWithRgba(first.animations, imgW, imgH, rgba);
  assert.deepEqual(second, first);
});

test('resolveCharacterAnimWithFlipX swaps left and right poses on flipX unless pre-swapped by descriptor', () => {
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: false, anim: 'left' }), 'left');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'left' }), 'right');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'right' }), 'left');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'missLeft' }), 'missRight');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'missRight' }), 'missLeft');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'up' }), 'up');
  assert.equal(resolveCharacterAnimWithFlipX({ flipX: true, anim: 'down' }), 'down');

  // Evita doble intercambio si character.json con flip_x ya asignó singLEFT -> Right y singRIGHT -> Left
  assert.equal(
    resolveCharacterAnimWithFlipX({
      flipX: true,
      anim: 'left',
      descriptorFlipX: true,
      leftPrefix: 'Pico Note Right',
      rightPrefix: 'Pico NOTE LEFT',
    }),
    'left',
  );
});

test('multi-camera slots resolve Cam 0 fixed, Cam 1 player, Cam 2 opponent, Cam 3+ custom and Lua V2 fixed mode', () => {
  const slots = defaultStageCameraSlots(640, 360, 1.0, 6.0);
  assert.equal(slots.length, 3);
  assert.equal(slots[0].targetKind, 'fixed');
  assert.equal(slots[1].targetKind, 'player');
  assert.equal(slots[2].targetKind, 'opponent');

  slots.push({
    index: 3,
    name: 'Cam 3 · Cabina',
    targetKind: 'gf',
    targetEntityId: null,
    x: 0,
    y: -30,
    zoom: 1.35,
    smoothSpeed: 8.0,
    panStrength: 1.0,
  });
  slots.push({
    index: 4,
    name: 'Cam 4 · Medio Camino Enemigo',
    targetKind: 'opponent',
    targetEntityId: null,
    x: 0,
    y: 0,
    zoom: 1.15,
    smoothSpeed: 5.0,
    panStrength: 0.5,
  });

  const cfg = {
    enabled: true,
    baseX: 640,
    baseY: 360,
    baseZoom: 1.0,
    followMode: 'fixed' as const,
    followIntervalBeats: 4,
    smoothSpeed: 6.0,
    panStrength: 0.85,
    activeCameraIndex: 3,
    cameras: slots,
  };
  const entities = [
    { id: 'stage_bg', role: 'background' as const, x: 640, y: 360, visible: true },
    { id: 'stage_speakers', displayName: 'Cabina de Sonido', role: 'decoration' as const, x: 640, y: 415, visible: true },
    { id: 'opponent_main', role: 'opponent' as const, x: 300, y: 415, visible: true, cameraOffset: [20, -10] as [number, number] },
    { id: 'player_main', role: 'player' as const, x: 980, y: 420, visible: true },
  ];

  const c0 = resolveCameraSlotTarget({ camera: cfg, entities, slotIndex: 0 });
  assert.deepEqual({ x: c0.x, y: c0.y, zoom: c0.zoom }, { x: 640, y: 360, zoom: 1.0 });

  const c1 = resolveCameraSlotTarget({ camera: cfg, entities, slotIndex: 1, activePlayerId: 'player_main' });
  assert.equal(c1.x, 980);
  assert.equal(c1.y, 420 - 24);

  const c2 = resolveCameraSlotTarget({ camera: cfg, entities, slotIndex: 2, activeOpponentId: 'opponent_main' });
  assert.equal(c2.x, 320); // 300 + cameraOffset[0]=20
  assert.equal(c2.y, 415 - 10 - 24);

  const c3 = resolveCameraSlotTarget({ camera: cfg, entities, slotIndex: 3 });
  assert.equal(c3.x, 640);
  assert.equal(c3.y, 385);
  assert.equal(c3.zoom, 1.35);

  const c4 = resolveCameraSlotTarget({ camera: cfg, entities, slotIndex: 4 });
  // panStrength 0.5 interpolates halfway between Cam 0 (640,360) and opponent (320,405)
  assert.equal(c4.x, 480);
  assert.ok(Math.abs(c4.y - 382.5) < 0.01);

  const luaBlock = renderCameraLuaConfigBlock(cfg);
  assert.ok(luaBlock.includes('local CAM_MODE = "fixed"'));
  assert.ok(luaBlock.includes('local CAM_ACTIVE_INDEX = 3'));
  assert.ok(luaBlock.includes('local CAM_COUNT = 5'));

  const parsedLua = parseCameraConfigFromLua(
    `${luaBlock}\nfunction onSongStart() switchCamera(3) end`,
    { ...cfg, followMode: 'sections', activeCameraIndex: 0 },
  );
  assert.equal(parsedLua.followMode, 'fixed');
  assert.equal(parsedLua.activeCameraIndex, 3);
});

test('chart.json preserves section.cameraSlot and parseChartCameraEvent handles V-Slice, Psych, Codename and Switch Camera', () => {
  const raw = JSON.stringify({
    song: {
      song: 'Cam Test',
      bpm: 150,
      speed: 2.0,
      notes: [
        { mustHitSection: false, cameraSlot: 0, sectionNotes: [[100, 0, 0]] },
        { mustHitSection: true, cameraSlot: 3, sectionNotes: [[1700, 0, 0]] },
      ],
    },
  });
  const parsed = parseChartJson(raw);
  assert.equal(parsed.sections[0].cameraSlot, 0);
  assert.equal(parsed.sections[1].cameraSlot, 3);
  const reserialized = parseChartJson(serializeChartJson(parsed));
  assert.equal(reserialized.sections[0].cameraSlot, 0);
  assert.equal(reserialized.sections[1].cameraSlot, 3);

  const sw = parseChartCameraEvent({
    name: 'Switch Camera',
    value1: 'cam=3;zoom=1.35;dur=0.50;ease=cubicOut',
    value2: '',
  });
  assert.deepEqual(sw, {
    kind: 'select_camera',
    slot: 3,
    focusGf: false,
    offsetX: 0,
    offsetY: 0,
    zoom: 1.35,
    durationSec: 0.5,
    ease: 'cubicOut',
  });

  // 150 BPM -> 1 step = 0.1s
  const focusPlayer = parseChartCameraEvent({
    name: 'FocusCamera',
    value1: JSON.stringify({ char: 0, x: 25, y: -15, duration: 4, ease: 'quadOut' }),
    value2: '',
    activeBpm: 150,
  });
  assert.equal(focusPlayer?.kind, 'select_camera');
  if (focusPlayer?.kind === 'select_camera') {
    assert.equal(focusPlayer.slot, 1);
    assert.equal(focusPlayer.offsetX, 25);
    assert.equal(focusPlayer.offsetY, -15);
    assert.ok(Math.abs(focusPlayer.durationSec - 0.4) < 0.001);
  }

  const focusStageOrigin = parseChartCameraEvent({
    name: 'FocusCamera',
    value1: JSON.stringify({ char: -1 }),
    value2: '',
  });
  assert.equal(focusStageOrigin?.kind, 'select_camera');
  if (focusStageOrigin?.kind === 'select_camera') {
    assert.equal(focusStageOrigin.slot, 0);
  }

  const zoomStage = parseChartCameraEvent({
    name: 'ZoomCamera',
    value1: JSON.stringify({ zoom: 1.25, duration: 8, mode: 'stage', ease: 'sineInOut' }),
    value2: '',
    activeBpm: 120,
  });
  assert.deepEqual(zoomStage, {
    kind: 'set_zoom',
    zoom: 1.25,
    isDirectStage: true,
    durationSec: 1.0,
    ease: 'sineInOut',
  });

  const lock = parseChartCameraEvent({ name: 'Camera Follow Pos', value1: '700', value2: '350' });
  assert.equal(lock?.kind, 'follow_pos');
  const unlock = parseChartCameraEvent({ name: 'Camera Follow Pos', value1: '', value2: '' });
  assert.equal(unlock?.kind, 'clear_follow_pos');

  const bump = parseChartCameraEvent({ name: 'Add Camera Zoom', value1: '0.04', value2: '0.02' });
  assert.deepEqual(bump, { kind: 'add_zoom_bump', camZoomAdd: 0.04, hudZoomAdd: 0.02 });

  const codenameOpp = parseChartCameraEvent({ name: 'Camera Movement', value1: '0', value2: '' });
  assert.equal(codenameOpp?.kind, 'select_camera');
  if (codenameOpp?.kind === 'select_camera') {
    assert.equal(codenameOpp.slot, 2);
  }

  const loopRes = optimizeScriptLoops(
    'lua',
    'for i = 1, 5000 do\n  switchCamera(3)\n  setCameraFollowPos(640, 360)\nend',
  );
  assert.equal(loopRes.discardedLoops, 0);
  assert.equal(loopRes.neutralizedLoops, 1);
  assert.ok(loopRes.optimizedSource.includes('switchCamera'));
});







