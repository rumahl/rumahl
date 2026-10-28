# Demo backend

The demo uses the production shell runtime, API client, contract validation, React components, and
state management. Only the HTTP and event transports are replaced by `DemoBackend` when
`pnpm dev:demo` is used. `publishSnapshot` emits the same revision event that the production
gateway will emit; the shell then fetches the new snapshot through the ordinary API client.
The demo widget is served at `127.0.0.1:5174`, separate from the shell's `localhost` host,
and is embedded through the same descriptor request and script-only sandbox as production.
No demo routes or widget content are included in the production client bundle.

When a production API operation is added:

1. Add it to the shared API client and shared contracts.
2. Call that client from the regular feature code.
3. Add the same HTTP route, state transition, and revision event to `DemoBackend`.
4. Test the feature through the shared runtime rather than importing demo data into production.

Production modules must never import from `src/demo`. The production bundle verification rejects
known demo markers, and the default development server blocks demo entry paths.
