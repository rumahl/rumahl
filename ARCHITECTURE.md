rumahl Core provides mechanisms, not products.

Apps never communicate directly with other apps.

Apps depend on capabilities, not concrete providers.

Permission request != permission grant.

Capability != contribution.

Every operation has an identity.

Resources are logical references, never host paths.

First-party apps use the same platform contracts as third-party apps.

Privileged OS operations are performed by isolated services.

Core must never contain app-specific behavior.

rumahl OS supports multiple local user accounts. Account selection and session
state are explicit; an active desktop session never changes the identity of an
existing app or OAuth session implicitly.

rumahl OS acts as its own standards-compliant OpenID Provider. Installed apps
can be registered as local OAuth 2.0 / OpenID Connect clients and use the OS
account for sign-in. Protocol DTOs, credentials, tokens, and signing keys stay
outside the domain core.

See [Account and OpenID Connect architecture](docs/architecture/account-and-oidc.md).
