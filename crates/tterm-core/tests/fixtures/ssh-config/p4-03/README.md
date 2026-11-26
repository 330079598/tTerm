# P4-03 shared SSH config import fixture

`config` and `included.conf` are the one synthetic OpenSSH configuration used
by both clients' import acceptance tests:

- `src-tauri/src/profiles/p4_03_acceptance_tests.rs` invokes the real Tauri
  `preview_ssh_config_import`, `import_ssh_config_profiles`, `list_profiles`
  and `list_tunnels` commands through Tauri's mock IPC.
- `crates/tterm-gpui/tests/p4_03_ssh_config_import.rs` calls the same
  core preview and import services the GPUI dialog calls.

Each test copies the templates to an isolated temporary directory and fills in
`__P4_03_INCLUDE__`, `__P4_03_KEY__` and `__P4_03_CERT__` with absolute paths
from that directory. The paths use `/` separators because the parser, like
`ssh`, treats `\` as an escape. Nothing reads the user's `~/.ssh/config`,
database or private keys.

The fixture deliberately contains:

- an `Include` whose `Match exec` line warns with the included file's absolute
  path and line 3;
- `focus`, the selected host, with a `ProxyJump` that has no username, key
  authentication, keepalive values and local, remote and dynamic forwards;
- `optional`, a second importable host that the first import does not select;
- `*.dropped`, a wildcard entry that the preview reports as skipped.