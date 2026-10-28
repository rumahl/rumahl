# rumahl UI contracts

`rumahl-ui-contracts` is the engine-neutral compatibility boundary between the
authoritative Rust platform and first-party or replaceable shell renderers.

The initial v1 contract provides:

- strict `ShellSnapshot` parsing with explicit snapshot, UI, and extension API
  versions;
- declarative theme manifests with an allowlisted token and variant set;
- deterministic CSS compilation from validated values;
- command, search-provider, and reserved dashboard-widget projections;
- checked-in JSON schemas, TypeScript bindings, and compatibility fixtures.

It deliberately contains no React, Node.js, HTTP, database, runtime-engine, or
privileged platform integration. Those adapters consume this crate rather than
becoming part of its public contract.
