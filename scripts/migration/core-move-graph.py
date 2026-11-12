#!/usr/bin/env python3
"""File-level dependency graph of src-tauri/src, for moving code into tterm-core (P1-09).

The graph is approximate: it follows `use crate::/super::/self::` trees and
inline `crate::`/`super::` paths outside `#[cfg(test)] mod … { … }` blocks and
comments, and maps each path to the file of its longest module prefix. It does
not see macros or glob re-exports. Use it to plan and to check, and let the
compiler have the last word.

    python3 -I scripts/migration/core-move-graph.py adapters   # logic files that still need an adapter file
    python3 -I scripts/migration/core-move-graph.py cycles     # strongly connected groups among movable files
    python3 -I scripts/migration/core-move-graph.py batches    # move batches, leaves first, per domain
    python3 -I scripts/migration/core-move-graph.py json OUT   # the whole graph
    add --no-tests to any command to leave test code out of the graph

A file is an *adapter* when its non-test code touches Tauri (see TAURI); it
stays in src-tauri. Test files and inline test modules move with the module
that owns them, so their dependencies count as the owner's, and a movable
module whose tests touch Tauri is reported as blocked too. Bare paths through
child modules (`legacy_import::x`) and through `use`-imported module names
count as dependencies. A file in STAY stays as well. Every other non-test file is
*movable*. `adapters` exits with status 1 while a movable file depends on an
adapter or a STAY file, which is the condition P1-09a must clear.
"""

import json
import os
import re
import sys
from collections import Counter, defaultdict

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "src-tauri", "src")

TAURI = re.compile(
    r"\btauri::|\bAppHandle\b|\btauri_plugin|#\[tauri::command|\bState<'|\bWebviewWindow\b|\bManager\b|\bEmitter\b"
)

# Files and directories that stay in src-tauri whole, whether or not they
# mention Tauri: startup, the event and session sinks, the platform adapters
# and the window glue (P1-07 section 1.3, P1-08).
STAY = (
    "lib.rs",
    "main.rs",
    "migrate.rs",
    "platform/",
    "event_sink.rs",
    "event_sink/",
    "session_sink.rs",
    "session_sink/",
    "app_paths.rs",
    "app_identity.rs",
    "main_window.rs",
    "window_blur.rs",
    "ime_focus.rs",
    "clipboard_files.rs",
    "drag_out/",
)

# Suggested domain order (task card P1-09); the rest follow alphabetically.
ORDER = (
    "config db profiles command_library ssh terminal serial zmodem sftp tunnel proxy monitor "
    "session_log backup sync"
).split()


def is_test(rel):
    base = os.path.basename(rel)
    return base == "tests.rs" or base.endswith("_tests.rs") or "/tests/" in "/" + rel


def is_stay(rel):
    return any(rel == entry or (entry.endswith("/") and rel.startswith(entry)) for entry in STAY)


def module_path(rel):
    parts = rel[:-3].split("/")
    if parts[-1] in ("mod", "lib", "main"):
        parts = parts[:-1]
    return tuple(parts)


def strip_tests(source):
    """Removes every `#[cfg(test)] mod name { … }` block by brace matching."""
    kept = []
    position = 0
    for match in re.finditer(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{", source):
        if match.start() < position:
            continue
        kept.append(source[position : match.start()])
        depth = 1
        cursor = match.end()
        while cursor < len(source) and depth:
            if source[cursor] == "{":
                depth += 1
            elif source[cursor] == "}":
                depth -= 1
            cursor += 1
        position = cursor
    kept.append(source[position:])
    return "".join(kept)


def strip_comments(source):
    source = re.sub(r"//[^\n]*", "", source)
    return re.sub(r"/\*.*?\*/", "", source, flags=re.S)


def expand_use_tree(prefix, tree):
    """Expands a `use` tree into (path, alias) pairs; alias is the name the
    item is known by in the importing file, or None for globs and `self`."""
    tree = tree.strip()
    if not tree:
        return []
    if tree.startswith("{") and tree.endswith("}"):
        found = []
        depth = 0
        current = ""
        for char in tree[1:-1]:
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
            if char == "," and depth == 0:
                found += expand_use_tree(prefix, current)
                current = ""
            else:
                current += char
        return found + expand_use_tree(prefix, current)
    if "::" in tree.split(" as ")[0]:
        head, rest = tree.split("::", 1)
        return expand_use_tree(prefix + [head.strip()], rest)
    parts = tree.split(" as ")
    name = parts[0].strip()
    alias = parts[1].strip() if len(parts) > 1 else name
    if name == "*":
        return [(prefix, None)]
    if name == "self":
        return [(prefix, alias if len(parts) > 1 else (prefix[-1] if prefix else None))]
    return [(prefix + [name], alias)]


def test_blocks(source):
    """The bodies of `#[cfg(test)] mod … { … }` blocks."""
    blocks = []
    for match in re.finditer(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{", source):
        depth = 1
        cursor = match.end()
        while cursor < len(source) and depth:
            if source[cursor] == "{":
                depth += 1
            elif source[cursor] == "}":
                depth -= 1
            cursor += 1
        blocks.append(source[match.end() : cursor])
    return blocks


def load(include_tests=True):
    """Builds the graph. Edges of a test file, and of inline test modules, are
    added to the module that owns them, since tests move with their module;
    tests that touch Tauri are reported in `test_adapters` by owner."""
    files = {}
    for directory, _, names in os.walk(ROOT):
        for name in names:
            if name.endswith(".rs"):
                full = os.path.join(directory, name)
                rel = os.path.relpath(full, ROOT).replace(os.sep, "/")
                with open(full, encoding="utf-8") as handle:
                    files[rel] = handle.read()
    modules = {module_path(rel): rel for rel in files}

    def resolve(path):
        for length in range(len(path), 0, -1):
            target = modules.get(tuple(path[:length]))
            if target:
                return target
        return None

    def owner(rel):
        path = list(module_path(rel))
        while path:
            candidate = modules.get(tuple(path))
            if candidate and not is_test(candidate):
                return candidate
            path = path[:-1]
        return rel

    def absolute_of(root, path, here):
        if root == "crate":
            return list(path)
        if root == "super":
            base = here[:-1]
            rest = list(path)
            while rest and rest[0] == "super":
                base = base[:-1]
                rest = rest[1:]
            return base + rest
        return here + list(path)

    def paths_in(body, here):
        found = []
        aliases = {}
        for use in re.finditer(r"\buse\s+(crate|super|self)::(.*?);", body, flags=re.S):
            for path, alias in expand_use_tree([], re.sub(r"\s+", " ", use.group(2))):
                absolute = absolute_of(use.group(1), path, here)
                found.append(absolute)
                if alias and tuple(absolute) in modules:
                    aliases[alias] = absolute
        for inline in re.finditer(
            r"\b(crate|super)::((?:[a-z_][a-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*)", body
        ):
            found.append(absolute_of(inline.group(1), inline.group(2).split("::"), here))
        # Child modules (`mod x;`) and module aliases used as bare paths, such
        # as `legacy_import::import_settings_files(..)`.
        children = {
            name: here + [name]
            for name in re.findall(r"\bmod\s+([a-z_][a-z0-9_]*)\s*;", body)
        }
        bare = {**children, **aliases}
        for inline in re.finditer(
            r"(?<![\w:])([a-z_][a-z0-9_]*)::((?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*)",
            body,
        ):
            head = inline.group(1)
            if head in bare and head not in ("crate", "super", "self"):
                found.append(bare[head] + inline.group(2).split("::"))
        return found

    edges = defaultdict(set)
    items = defaultdict(set)
    adapters = set()
    test_adapters = defaultdict(set)
    for rel, source in files.items():
        plain = strip_comments(source)
        here = list(module_path(rel))
        if is_test(rel):
            if not include_tests:
                continue
            bodies = [plain]
        else:
            body = strip_tests(plain)
            if TAURI.search(body):
                adapters.add(rel)
            bodies = [body] + (test_blocks(plain) if include_tests else [])
        home = owner(rel) if is_test(rel) else rel
        for index, body in enumerate(bodies):
            in_test = is_test(rel) or index > 0
            if in_test and TAURI.search(body):
                test_adapters[home].add(rel)
            for absolute in paths_in(body, here):
                target = resolve(absolute)
                if target and is_test(target):
                    target = owner(target)
                if target and target != home:
                    edges[home].add(target)
                    if absolute:
                        items[(home, target)].add(absolute[-1])
    movable = {
        rel for rel in files if rel not in adapters and not is_test(rel) and not is_stay(rel)
    }
    return files, edges, items, adapters, movable, test_adapters


def domain(rel):
    return rel.split("/")[0].removesuffix(".rs")


def strongly_connected(nodes, edges):
    sys.setrecursionlimit(100000)
    index, low, stack, on_stack, found = {}, {}, [], set(), []
    counter = [0]

    def visit(node):
        index[node] = low[node] = counter[0]
        counter[0] += 1
        stack.append(node)
        on_stack.add(node)
        for target in edges.get(node, ()):
            if target not in nodes:
                continue
            if target not in index:
                visit(target)
                low[node] = min(low[node], low[target])
            elif target in on_stack:
                low[node] = min(low[node], index[target])
        if low[node] == index[node]:
            group = []
            while True:
                top = stack.pop()
                on_stack.discard(top)
                group.append(top)
                if top == node:
                    break
            found.append(sorted(group))

    for node in sorted(nodes):
        if node not in index:
            visit(node)
    return found


def closure(start, moved, movable, edges):
    batch = set()
    pending = list(start)
    while pending:
        rel = pending.pop()
        if rel in batch or rel in moved:
            continue
        batch.add(rel)
        pending += [target for target in edges.get(rel, ()) if target in movable]
    return batch


def batches(movable, edges):
    """Greedy batches: per domain, the files whose dependencies are already
    moved or in the same domain; when no domain can advance, the smallest
    cross-domain group that must move together."""
    order = ORDER + sorted({domain(rel) for rel in movable} - set(ORDER))
    moved = set()
    result = []
    while len(moved) < len(movable):
        advanced = False
        for name in order:
            ready = {
                rel
                for rel in movable
                if domain(rel) == name
                and rel not in moved
                and all(domain(other) == name for other in closure({rel}, moved, movable, edges))
            }
            if ready:
                moved |= ready
                result.append((name, sorted(ready)))
                advanced = True
        if not advanced:
            rest = movable - moved
            seed = min(sorted(rest), key=lambda rel: len(closure({rel}, moved, movable, edges)))
            group = closure({seed}, moved, movable, edges)
            moved |= group
            result.append(("cross-domain", sorted(group)))
    return result


def main():
    arguments = [argument for argument in sys.argv[1:] if argument != "--no-tests"]
    command = arguments[0] if arguments else "adapters"
    files, edges, items, adapters, movable, test_adapters = load("--no-tests" not in sys.argv)
    blocked = {rel for rel in files if (rel in adapters or is_stay(rel)) and not is_test(rel)}
    if command == "adapters":
        needed = defaultdict(lambda: defaultdict(set))
        for (source, target), names in items.items():
            if source in movable and target in blocked:
                needed[target][source] |= names
        for target in sorted(needed, key=lambda key: (-len(needed[key]), key)):
            print(f"{target}  ({len(needed[target])} movable dependents)")
            for source in sorted(needed[target]):
                print(f"    {source}: {' '.join(sorted(needed[target][source]))}")
        blocked_tests = {home: tests for home, tests in test_adapters.items() if home in movable}
        for home in sorted(blocked_tests):
            print(f"{home}  (its tests use Tauri: {' '.join(sorted(blocked_tests[home]))})")
        print(
            f"{len(movable)} movable files, {len(needed)} adapter or stay files still needed, "
            f"{len(blocked_tests)} movable files with Tauri tests"
        )
        sys.exit(1 if needed or blocked_tests else 0)
    if command == "cycles":
        for group in strongly_connected(movable, edges):
            if len(group) > 1:
                print(f"{len(group)} files: {' '.join(group)}")
        return
    if command == "batches":
        for number, (name, group) in enumerate(batches(movable, edges), start=1):
            domains = Counter(domain(rel) for rel in group)
            print(f"{number:2}. {name}: {len(group)} files {dict(domains)}")
            for rel in group:
                print(f"      {rel}")
        return
    if command == "json":
        with open(arguments[1], "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "adapters": sorted(adapters),
                    "movable": sorted(movable),
                    "edges": {rel: sorted(targets) for rel, targets in sorted(edges.items())},
                },
                handle,
                indent=1,
            )
        return
    sys.exit(f"unknown command {command!r}")


if __name__ == "__main__":
    main()
