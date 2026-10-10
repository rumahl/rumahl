# rumahl OS Architecture

rumahl OS is a local-first operating system and application platform for the home.

This document provides a high-level overview of the system, its main architectural boundaries, and where to start when working on the codebase.

Detailed subsystem documentation lives under [`docs/architecture/`](docs/architecture/).

## Principles

The architecture follows a small set of platform-wide invariants:

- rumahl Core provides mechanisms, not products.
- Core must never contain app-specific behavior.
- Apps never communicate directly with other apps.
- Apps depend on capabilities, not concrete providers.
- Permission request != permission grant.
- Capability != contribution.
- Every operation has an identity.
- Resources are logical references, never host paths.
- First-party apps use the same platform contracts as third-party apps.
- Privileged OS operations are performed by isolated services.
- Transport, persistence, authentication, and UI concerns stay outside the domain core.

These rules define architectural boundaries rather than implementation details.

## System overview

```text
┌─────────────────────────────────────┐
│          Frontend / OS Shell        │
│              frontend/              │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│        Platform Web / API           │
│  platform-web/ · platform-api/      │
└──────────────────┬──────────────────┘
                   │
                   ▼
┌─────────────────────────────────────┐
│          Platform Service           │
│         platform-service/           │
└───────┬─────────┬─────────┬─────────┘
        │         │         │
        ▼         ▼         ▼
   Authentication   App      Persistence
 account-auth/   operations/ persistence-sqlite/
 oidc-provider/
        │         │         │
        └─────────┴────┬────┘
                       ▼
┌─────────────────────────────────────┐
│             Domain Core             │
│                core/                │
└─────────────────────────────────────┘
```

The frontend and applications do not directly perform privileged host operations.

Requests pass through platform contracts and services where identity, permissions, resource ownership, and platform policy can be enforced.

## Source map

### `core/`

Domain concepts, policies, and platform invariants.

Core should not depend on HTTP, SQLite, frontend frameworks, OAuth/OIDC protocol types, host paths, or app-specific behavior.

### `platform-api/`

Stable contracts used to expose platform functionality without leaking implementation details.

### `platform-service/`

The main composition layer.

It connects Core with concrete infrastructure such as authentication, persistence, application operations, and web-facing services.

### `platform-web/`

Web and HTTP-facing integration.

Transport-specific concepts stay here rather than leaking into Core.

### `frontend/`

The rumahl OS shell and frontend packages.

The shell owns fundamental OS-level UI behavior while consuming explicit platform contracts.

### `ui-contracts/`

Stable contracts shared between platform functionality and UI implementations.

### `account-auth/`

Authentication infrastructure for local rumahl OS accounts.

### `oidc-provider/`

OAuth 2.0 / OpenID Connect provider used by installed applications to authenticate against local rumahl OS accounts.

### `app-operations/`

Platform-mediated operations requested by applications.

Applications request operations through the platform instead of accessing privileged host functionality directly.

### `persistence-sqlite/`

Current SQLite-backed persistence implementation.

Database-specific representations must remain outside the domain model.

### `platform-buildroot/`

Integration of rumahl OS platform components into the system image.

Buildroot is an implementation detail of the OS image and not part of the application-facing platform API.

## Important boundaries

### Core vs. infrastructure

Core describes platform concepts and rules.

Infrastructure modules implement those concepts using technologies such as SQLite, HTTP, OAuth/OIDC, or host operating-system services.

Dependencies should point toward platform abstractions rather than infrastructure leaking into Core.

```text
UI / Transport
      │
      ▼
Application / Platform Services
      │
      ▼
Domain Core
```

### Applications vs. the host

Applications operate through platform capabilities.

They should not receive unrestricted access to host paths, privileged operating-system APIs, or another application's internal resources.

### Identity

Every relevant operation has an identity.

The platform must be able to determine which user, application, or service is responsible for an operation before privileged work is performed.

### Permissions

Declaring or requesting a capability does not grant permission to use it.

Permission decisions belong to the platform.

### Resources

Applications reference logical resources instead of physical host paths.

The platform resolves those resources and controls access to them.

## Accounts and sessions

rumahl OS supports multiple local user accounts.

Account selection and session state are explicit. Changing the active desktop account must not implicitly change the identity of an existing app or OAuth session.

rumahl OS also acts as its own standards-compliant OpenID Provider.

Installed applications may use OAuth 2.0 / OpenID Connect to authenticate users through the local OS account.

Protocol DTOs, credentials, tokens, and signing keys stay outside Core.

See [Account and OpenID Connect architecture](docs/architecture/account-and-oidc.md).

## Where to start

For platform behavior and domain rules:

- [`core/`](core/)
- [`platform-api/`](platform-api/)

For backend execution and composition:

- [`platform-service/`](platform-service/)
- [`platform-web/`](platform-web/)

For the OS interface:

- [`frontend/`](frontend/)
- [`ui-contracts/`](ui-contracts/)

For authentication:

- [`account-auth/`](account-auth/)
- [`oidc-provider/`](oidc-provider/)

For application/platform interaction:

- [`app-operations/`](app-operations/)

For persistence:

- [`persistence-sqlite/`](persistence-sqlite/)

For system image integration:

- [`platform-buildroot/`](platform-buildroot/)

For local development:

- [Development guide](docs/development.md)

## Detailed architecture

Subsystem-specific decisions and implementation details are documented separately:

- [Account and OpenID Connect](docs/architecture/account-and-oidc.md)
- [App databases](docs/architecture/app-databases.md)
- [App hosting](docs/architecture/app-hosting.md)
- [App resource operations](docs/architecture/app-resource-operations.md)
- [Buildroot acceptance](docs/architecture/buildroot-acceptance.md)
- [Buildroot migration](docs/architecture/buildroot-migration.md)
- [Filesystem layout](docs/architecture/filesystem-layout.md)
- [Injection safety](docs/architecture/injection-safety.md)
- [Secret store](docs/architecture/secret-store.md)
- [Shell preferences](docs/architecture/shell-preferences.md)
- [Shell routing](docs/architecture/shell-routing.md)
- [Storage and updates](docs/architecture/storage-and-updates.md)