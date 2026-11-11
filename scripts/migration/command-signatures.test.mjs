import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { collectSignatures } from './command-signatures.mjs';

const baselinePath = path.resolve('scripts/migration/command-signatures.json');

test('command signatures match the checked in baseline', () => {
  const actual = collectSignatures();
  const expected = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  assert.deepStrictEqual(actual, expected);
});

test('command signatures include the generated handler list', () => {
  const signatures = collectSignatures();
  assert.ok(signatures.commands.length > 0);
  assert.ok(signatures.handlers.length > 0);
  assert.equal(new Set(signatures.handlers).size, signatures.handlers.length);
});
