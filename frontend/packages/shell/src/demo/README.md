# Demo backend

The demo uses the production shell runtime, API client, contract validation, React components, and
state management. Only the HTTP transport is replaced by `DemoBackend` when `pnpm dev:demo` is
used.

When a production API operation is added:

1. Add it to the shared API client and shared contracts.
2. Call that client from the regular feature code.
3. Add the same HTTP route and state transition to `DemoBackend`.
4. Test the feature through the shared runtime rather than importing demo data into production.

Production modules must never import from `src/demo`. The production bundle verification rejects
known demo markers, and the default development server blocks demo entry paths.
