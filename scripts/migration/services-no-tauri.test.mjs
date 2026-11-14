import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const servicesRoot = path.resolve('crates/tterm-core/src/services');
function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? rustFiles(file) : entry.name.endsWith('.rs') ? [file] : [];
  });
}
test('services contain no Tauri adapter types', () => {
  for (const file of rustFiles(servicesRoot)) {
    assert.doesNotMatch(fs.readFileSync(file, 'utf8'), /\btauri::|\bAppHandle\b|\bState\s*</, file);
  }
});
