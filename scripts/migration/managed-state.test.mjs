import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// P1-07s changed these managed values to Arc<T>; bare T compiles but panics
// when Tauri injects it or state() looks it up at runtime.
const arcManaged = [
  'PendingMonitorPrompts',
  'ExternalEditState',
  'SessionLogState',
  'TunnelManager',
  'CloseBehaviorState',
  'QuitState',
  'ExitGuardState',
  'LaunchDirs',
  'TrayState',
  'PendingUpdateDownloads',
];

// P1-07b, d and e must update this list whenever they remove an individual
// manage registration. These values must then be read through AppServices.
const retired = [
  'TransferCancelMap',
  'ExternalEditState',
  'CloseBehaviorState',
  'QuitState',
  'ExitGuardState',
  'TrayState',
  'LaunchDirs',
  'PendingUpdateDownloads',
  'Notifications',
  'IntegrationState',
  'ZmodemMap',
  'ZmodemArmedSendMap',
  'ZmodemManualDetectMap',
  'MonitorSessionMap',
  'PendingMonitorPrompts',
];

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? rustFiles(file) : entry.name.endsWith('.rs') ? [file] : [];
  });
}

function violations(source) {
  const errors = [];
  const start = /\b(State\s*<|(?:try_state|state)\s*::\s*<)/g;
  // Ignore documentation and commented-out examples, preserving line numbers.
  const code = source.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, (text) => text.replace(/[^\n]/g, ' '));
  for (const match of code.matchAll(start)) {
    const open = match.index + match[0].length - 1;
    let end = open + 1;
    let depth = 1;
    while (end < code.length && depth > 0) {
      if (code[end] === '<') depth += 1;
      if (code[end] === '>') depth -= 1;
      end += 1;
    }
    const type = code.slice(open + 1, end - 1).replace(/^\s*'\w+\s*,/, '').trim();
    const line = code.slice(0, match.index).split('\n').length;
    for (const name of retired) {
      if (new RegExp(`\\b${name}\\b`).test(type)) errors.push(`${line}: retired managed state ${type}`);
    }
    for (const name of arcManaged) {
      if (new RegExp(`^(?:\\w+\\s*::\\s*)*${name}$`).test(type)) {
        errors.push(`${line}: managed state requires Arc<${name}>`);
      }
    }
  }
  return errors;
}

test('managed state scanner catches injection and lookup regressions', () => {
  for (const name of arcManaged) {
    for (const usage of [
      `State<'_, crate::domain::${name}>`,
      `app.state::<${name}>()`,
      `app.try_state::<\n crate::domain::${name}\n>()`,
    ]) assert.ok(violations(usage).length > 0, usage);
  }
  for (const name of retired) {
    for (const usage of [`State<'_, Arc<${name}>>`, `app.try_state::<${name}>()`]) {
      assert.ok(violations(usage).length > 0, usage);
    }
  }
  for (const name of arcManaged.filter((name) => !retired.includes(name))) {
    assert.deepEqual(violations(`State<'_, std::sync::Arc<crate::domain::${name}>>`), []);
    assert.deepEqual(violations(`app.state::<Arc<${name}>>()`), []);
  }
  assert.deepEqual(violations("// State<'_, TunnelManager>\n/* state::<ZmodemMap>() */"), []);
  assert.deepEqual(violations("State<'_, Arc<AppServices>>"), []);
});

test('Tauri state readers match the remaining managed registrations', () => {
  const root = fileURLToPath(new URL('../../src-tauri/src/', import.meta.url));
  const errors = rustFiles(root).flatMap((file) =>
    violations(fs.readFileSync(file, 'utf8')).map((error) => `${path.relative(root, file)}:${error}`),
  );
  assert.deepEqual(errors, []);
});
