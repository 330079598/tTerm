#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../..');
const sourceRoot = path.join(root, 'src-tauri', 'src');
const baselinePath = path.join(root, 'scripts', 'migration', 'command-signatures.json');

function splitTopLevel(input, separator = ',') {
  const parts = [];
  let start = 0;
  let angle = 0;
  let paren = 0;
  let bracket = 0;
  let brace = 0;
  let string = null;
  let escaped = false;
  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];
    if (string) {
      if (escaped) escaped = false;
      else if (char === '\\') escaped = true;
      else if (char === string) string = null;
      continue;
    }
    if (char === '"' || (char === "'" && !/^'[A-Za-z_][A-Za-z0-9_]*(?![A-Za-z0-9_'])/.test(input.slice(index)))) {
      string = char;
      continue;
    }
    if (char === '<') angle += 1;
    else if (char === '>') angle = Math.max(0, angle - 1);
    else if (char === '(') paren += 1;
    else if (char === ')') paren = Math.max(0, paren - 1);
    else if (char === '[') bracket += 1;
    else if (char === ']') bracket = Math.max(0, bracket - 1);
    else if (char === '{') brace += 1;
    else if (char === '}') brace = Math.max(0, brace - 1);
    else if (char === separator && angle === 0 && paren === 0 && bracket === 0 && brace === 0) {
      parts.push(input.slice(start, index).trim());
      start = index + 1;
    }
  }
  const tail = input.slice(start).trim();
  if (tail) parts.push(tail);
  return parts;
}

function findMatchingBrace(source, openIndex) {
  let depth = 0;
  let string = null;
  let escaped = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const char = source[index];
    if (string) {
      if (escaped) escaped = false;
      else if (char === '\\') escaped = true;
      else if (char === string) string = null;
      continue;
    }
    if (char === '"' || char === "'") {
      string = char;
      continue;
    }
    if (char === '{') depth += 1;
    else if (char === '}' && --depth === 0) return index;
  }
  return source.length;
}

function findMatchingBracket(source, openIndex) {
  let depth = 0;
  let string = null;
  let escaped = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const char = source[index];
    if (string) {
      if (escaped) escaped = false;
      else if (char === '\\') escaped = true;
      else if (char === string) string = null;
      continue;
    }
    if (char === '"' || char === "'") {
      string = char;
      continue;
    }
    if (char === '[') depth += 1;
    else if (char === ']' && --depth === 0) return index;
  }
  return source.length;
}

function parseParameter(parameter) {
  const withoutAttributes = parameter.replace(/^\s*#\[[\s\S]*?\]\s*/, '').trim();
  const colon = withoutAttributes.indexOf(':');
  if (colon < 0) return null;
  const name = withoutAttributes.slice(0, colon).trim().replace(/^mut\s+/, '');
  const type = withoutAttributes.slice(colon + 1).trim();
  if (!name || !type) return null;
  const injected = /(?:^|::)(?:State|AppHandle|Window|WebviewWindow|Webview)\s*(?:<|\b)/.test(type)
    || /^(?:tauri::)?(?:State|AppHandle|Window|WebviewWindow|Webview)\b/.test(type);
  return injected ? null : { name, type };
}

export function parseCommands(source) {
  const commands = [];
  const commandAttribute = /#\[tauri::command(?:\s*\(([^\]]*)\))?\]/g;
  let attributeMatch;
  while ((attributeMatch = commandAttribute.exec(source))) {
    const afterAttribute = source.slice(attributeMatch.index + attributeMatch[0].length);
    const functionMatch = afterAttribute.match(/\bpub\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^{}]*>)?\s*\(([^)]*)\)\s*(?:->\s*([^\{]+?))?\s*\{/s);
    if (!functionMatch) continue;
    const [, name, rawParameters, rawReturn] = functionMatch;
    const attributes = {};
    const rename = attributeMatch[1]?.match(/rename_all\s*=\s*"([^"]+)"/);
    if (rename) attributes.rename_all = rename[1];
    const parameters = splitTopLevel(rawParameters).map(parseParameter).filter(Boolean);
    commands.push({
      name,
      attributes,
      params: parameters,
      returns: rawReturn?.trim() ?? '()',
    });
  }
  return commands;
}

function collectRustFiles(directory) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectRustFiles(file));
    else if (entry.isFile() && entry.name.endsWith('.rs')) files.push(file);
  }
  return files;
}

function parseHandlers(source) {
  const match = source.match(/tauri::generate_handler!\s*\[/);
  if (!match) return [];
  const open = match.index + match[0].length - 1;
  const close = findMatchingBracket(source, open);
  return splitTopLevel(source.slice(open + 1, close))
    .map((entry) => {
      let path = entry.trim();
      while (path.startsWith('#[')) {
        path = path.slice(findMatchingBracket(path, 1) + 1).trim();
      }
      return path.replace(/\/\/.*$/gm, '').trim().split('::').at(-1);
    })
    .filter(Boolean);
}

export function collectSignatures() {
  const commands = [];
  let handlers = [];
  for (const file of collectRustFiles(sourceRoot)) {
    const source = fs.readFileSync(file, 'utf8');
    commands.push(...parseCommands(source));
    if (file === path.join(sourceRoot, 'lib.rs')) handlers = parseHandlers(source);
  }
  commands.sort((left, right) => left.name.localeCompare(right.name));
  handlers.sort();
  return { version: 1, commands, handlers };
}

function main() {
  const signatures = collectSignatures();
  if (process.argv.includes('--check')) {
    if (!fs.existsSync(baselinePath)) {
      console.error(`Missing baseline: ${path.relative(root, baselinePath)}`);
      process.exitCode = 1;
      return;
    }
    const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
    if (JSON.stringify(signatures) !== JSON.stringify(baseline)) {
      console.error('Command signatures changed. Regenerate only when the IPC contract intentionally changes.');
      process.exitCode = 1;
    }
    return;
  }
  fs.writeFileSync(baselinePath, `${JSON.stringify(signatures, null, 2)}\n`);
  console.log(`Wrote ${path.relative(root, baselinePath)} (${signatures.commands.length} commands)`);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
