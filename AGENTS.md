# AGENTS.md

Rules for anyone changing this repository: people, Claude Code, Codex,
opencode, or any other coding agent. Where an instruction conflicts with a
habit or default of yours, this file wins.

## Project

tTerm is a desktop SSH terminal built on Tauri 2.

- `src/`: the frontend, written in React 19 and TypeScript, built with Vite and styled with Tailwind.
- `src-tauri/src/`: the Rust backend, which handles PTY, SSH, SFTP, tunnels, backup, sync and the database.
- The frontend calls the backend through `#[tauri::command]` functions and `invoke`.
- Use pnpm, never npm or yarn. Run every `cargo` command from `src-tauri/`.

## Before you finish

Run these checks and make them pass. CI runs the same ones in
`.github/workflows/check.yml`, and it runs clippy and the tests on Linux,
Windows and macOS.

```sh
pnpm lint && pnpm format:check && pnpm typecheck && pnpm test && pnpm check:size
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

- Don't silence a failing check to get it green. That means no `--no-verify`,
  no `eslint-disable`, no blanket `#[allow]`, and no raising a limit or a
  baseline entry. Fix the cause.
- If you can't fix it, stop and say so.

## Size limits

Big files are the main maintenance problem in this codebase. The limits are
enforced. Test files and generated files are exempt.

| What | Limit | Enforced by |
| --- | --- | --- |
| `.rs` file | 800 lines | `pnpm check:size` |
| `.ts` / `.tsx` file | 500 lines | `pnpm check:size` |
| `.css` file | 600 lines | `pnpm check:size` |
| Rust function | 150 lines | clippy `too_many_lines` (threshold in `src-tauri/clippy.toml`) |
| Rust function arguments | 7 | clippy `too_many_arguments` |
| React component | about 300 lines, or about 10 hook calls | review |

### Files that are already too big

- `scripts/file-size-baseline.json` lists the files that were already over the
  limit when it came in. A listed file may shrink but must never grow.
- After you shrink one, run `node scripts/check-file-size.mjs --update` to
  lower its entry. That command never raises an entry or adds a new one.
- If a listed file must grow, split it first.
- Rust functions that were already too long carry
  `#[expect(clippy::too_many_lines, reason = ...)]`. The `expect` turns into
  an error once the function is short enough, so delete the attribute then.
- Never add new `expect` or `allow` attributes for length.

### Splitting files

When you touch a big file, leave it smaller than you found it. Don't add a
feature to a file that is already over its limit; split the file first, in a
separate commit.

- **Rust modules.** `mod.rs` holds the module's types, constants and the
  `#[tauri::command]` entry points. The logic goes in submodules, one per
  responsibility. `src-tauri/src/backup/` is the model: `archive`, `export`,
  `apply`, `snapshot`, `diff`, `automatic`, and `tests.rs`.
- **Rust tests.** When a module's inline `mod tests` grows past about 200
  lines, move it to a sibling `tests.rs` file and declare it with
  `#[cfg(test)] mod tests;`.
- **React components.** Move state and effects into custom hooks named
  `useXxx.ts`, one hook per concern, the way `TerminalTab/useTerminalLifecycle.ts`
  does. Move pure logic into plain `.ts` modules with their own tests. Move
  each subview into its own component file.
- **CSS.** Split stylesheets along with the components they style, or use
  Tailwind classes.

## Where code goes

### Frontend

- `src/components/<Feature>/index.tsx` exports the feature. Its subcomponents,
  hooks and helpers live next to it in the same folder.
- `src/components/ui/` holds the shared primitives (shadcn style).
- `src/lib/` holds pure logic with no React. A module there gets a
  `*.test.ts` beside it.
- `src/hooks/` holds hooks shared by more than one feature.
- `src/contexts/` holds app-wide React contexts.
- `src/types/` holds types shared across features.

### Backend

- There is one module per domain: `ssh`, `sftp`, `tunnel`, `backup`, `sync`,
  `profiles`, `terminal`, and so on.
- `lib.rs` registers the commands and must not hold domain logic.

## Rust conventions

- **Too many arguments.** When a function needs more than 7 arguments, group
  them in a struct. Examples are `ConnectContext` and `SshTarget` in
  `ssh/jump/connect.rs`, and `DeleteBatch` in `sftp/internal/api/delete/batch.rs`.
- **Tauri commands are the exception.** A command's arguments are its IPC
  fields, so a command may exceed the limit with
  `#[allow(clippy::too_many_arguments, reason = "...")]`.
- **Blocking work.** A synchronous `#[tauri::command]` runs on the main thread
  and freezes the window. Make the command `async` and move file, database,
  keychain or crypto work into `core::blocking::run_blocking`.
- **Errors.** Commands return `Result<T, String>`, which matches the rest of
  the codebase. Write messages the user can act on, and don't add a new error
  style in only one module.
- **No panics in new production code.** Use `?` or a handled error instead
  of `unwrap()` and `expect()` outside tests.
- **Imports.** Import each item from the module that defines it. Avoid
  `pub use` re-exports, which give one item two paths.

## TypeScript and React conventions

- **Imports.** Import with the `@/` alias. ESLint enforces this. Don't add
  re-export files: `@/components/Foo` already resolves to
  `components/Foo/index.tsx`.
- **No `any`.** Type the data that crosses `invoke` on both sides.
- **Text.** Send user-facing text through i18next. Add every key to both
  `src/i18n/locales/en.json` and `src/i18n/locales/zh.json`.
- **Formatting.** Prettier decides formatting (`pnpm format`). Don't
  hand-format against it.

## Comments

- Comments explain why. The code already says what it does.
- Write them as full sentences.
- Write doc comments on public items and on any item whose purpose isn't
  obvious from its name.

## Commits

- Start the summary with a gitmoji code that fits the change: `:sparkles:`,
  `:bug:`, `:zap:`, `:recycle:`, `:white_check_mark:` and so on. Follow it with
  what changed and why.
- Make one logical change per commit. A refactor that moves code goes in its
  own commit, apart from any behavior change.
- Don't add `Co-Authored-By` trailers or any other AI-attribution lines to
  commits or pull requests.
