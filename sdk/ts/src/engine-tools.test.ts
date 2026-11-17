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
  autoBindSparrowPrefixes,
  compareNaturalFrameNames,
  computeCountdownState,
  computeTrimmedFramePlacement,
  computeUniformCharacterDrawBox,
  detectSpriteJsonFormat,
  mapRotatedSpriteSample,
  packPosesIntoBoundedAtlas,
  parseAstcHeader,
  parseChartJson,
  serializeChartJson,
  shouldDiscardInvisibleEntity,
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


