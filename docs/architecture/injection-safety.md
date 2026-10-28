# Injection safety

rumahl OS treats every external value as data, never as code or structure. The
rules below apply to each layer that reaches a privileged boundary.

## SQL

Every statement uses bound parameters; no query is assembled by interpolating
user input. The one private schema helper that builds DDL by interpolation
(`ensure_column`) restricts the table and column names to a safe identifier set
and only ever receives compile-time constants.

## Processes

Privileged tools (Docker, `tpm2_unseal`) are invoked with `Command` directly —
never through a shell — with the executable pinned to an absolute path, the
arguments passed as an array, and a cleared environment. No user-controlled
value is concatenated into a command line, so shell metacharacters have no
effect.

## Paths

Logical package paths reject `.`, `..`, absolute paths and separators. The OS
path layout deliberately uses real host paths and allows `..`, so the file
provider must **canonicalize** a requested path and then enforce the caller's
grant boundary *after* canonicalization. Nothing may escape the granted root
through `..` or through symlinks. See [filesystem layout](filesystem-layout.md).

## HTTP and JSON

Request bodies are bounded and parsed with an allow-listed field set; unexpected
keys are rejected rather than ignored. Schema, scope, revision and mode values
are validated against closed sets. Shell output escapes snapshot data
(`escapeJsonForHtml`) and uses a nonce-based Content Security Policy.

## Terminal (planned)

The terminal must never hand a string to a shell. It runs an argv array through a
bounded, authorized command layer, and the acting identity always comes from the
session, never from the command itself.
