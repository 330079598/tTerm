import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { collectSignatures, parseCommands } from './command-signatures.mjs';

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

// Lifetimes are syntax inside injected types, not quoted string contents.
test('parameters after injected state retain their IPC names and types', () => {
  const commands = parseCommands(`
    #[tauri::command]
    pub async fn save_profile(
      services: tauri::State<'_, Arc<AppServices>>,
      mut profile: SavedProfile,
      typed_password_tab_id: Option<String>,
    ) -> Result<(), String> { Ok(()) }
  `);
  assert.deepStrictEqual(commands[0].params, [
    { name: 'profile', type: 'SavedProfile' },
    { name: 'typed_password_tab_id', type: 'Option<String>' },
  ]);
});

test('moving a command to another file keeps its signature', () => {
  const command = `
    #[tauri::command]
    pub async fn save_profile(profile: SavedProfile) -> Result<(), String> {
      Ok(())
    }
  `;
  const inOldFile = `use crate::profiles::SavedProfile;\n${command}`;
  const inNewFile = `mod commands {\n${command}\n}`;

  assert.deepStrictEqual(parseCommands(inOldFile), parseCommands(inNewFile));
});

test('changing a command parameter type changes its signature', () => {
  const source = (type) => `
    #[tauri::command]
    pub fn resize(tab_id: String, size: ${type}) {}
  `;

  assert.notDeepStrictEqual(parseCommands(source('u16')), parseCommands(source('u32')));
});
