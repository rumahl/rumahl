# `@rumahl/contracts`

Checked-in TypeScript projections of the authoritative Rust UI contracts.
`src/v1.ts` is verified byte-for-byte by the `rumahl-ui-contracts` test suite.
It must not gain shell implementation details or authorization logic.

`src/events.ts` is the bounded browser-side parser for the `ShellEvent` enum in
`ui-contracts/src/event.rs`. Events contain only a revision hint or session-revocation signal;
the browser re-fetches the authoritative user-scoped snapshot over the authenticated API.

`src/widget.ts` validates a per-contribution frame descriptor returned by the trusted gateway.
The gateway must resolve the installed app to a distinct HTTPS origin, authorize that contribution
for the current user, and include the exact origin in SSR's CSP allowlist. A descriptor is never
an authorization token; widgets receive no shell cookie or privileged broker channel.
