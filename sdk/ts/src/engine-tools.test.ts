/**
 * Archivo: sdk/ts/src/engine-tools.test.ts
 * Propósito: Suite de pruebas unitarias (`node:test`) que valida las reglas
 * clave del motor: parseo de charts JSON, auto-armado Sparrow XML,
 * límite de 3 pistas de música y descarte de assets invisibles al compilar en caliente.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  autoBindSparrowPrefixes,
  parseChartJson,
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
