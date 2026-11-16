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
  computeCountdownState,
  detectSpriteJsonFormat,
  parseAstcHeader,
  parseChartJson,
  serializeChartJson,
  shouldDiscardInvisibleEntity,
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
