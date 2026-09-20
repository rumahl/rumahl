# Secret store

Status: engine-neutral core contract, encrypted SQLite adapter, and recoverable
OIDC client-secret registration implemented; Buildroot root-key provider and
runtime delivery acknowledgement remain.

## Boundary

`SecretStore` persists installation-bound secret material separately from the
platform snapshot, app manifest, operation journal, and normal app storage.
Every record contains:

- a random UUIDv7 `SecretId`;
- the complete owning `AppIdentity`;
- a validated namespaced `SecretPurpose`;
- zeroizing `SecretValue` material;
- its creation timestamp.

The pair `(InstallationId, SecretPurpose)` is unique. This makes recovery
lookups deterministic without exposing the value or placing a secret handle in
the app manifest.

## SQLite encryption

`SqliteSecretStore` encrypts every value independently with AES-256-GCM and a
random 96-bit nonce. The authentication data includes a format version, secret
ID, complete app identity, purpose, key ID, and creation timestamp. Modifying
those columns therefore causes authenticated decryption to fail rather than
returning a value under the wrong identity.

SQLite stores only:

```text
metadata + key ID + nonce + authenticated ciphertext
```

The 256-bit root key is supplied by `SecretEncryptionKeyProvider` and never
written to the database. The active key encrypts new values; historical keys
remain addressable by key ID for reads and later rotation. Missing keys and
authentication failures fail closed.

The local test provider uses an in-memory key only. Production Buildroot must
provide a device-bound key source, preferably TPM-backed where hardware allows,
and define backup/recovery behavior before encrypted secrets are relied upon.

## Lifecycle

Secret values are zeroized when their domain wrappers are dropped, and debug
output exposes only non-secret metadata. Removal by `InstallationId` is one
atomic store statement. Higher-level app operations still decide whether an
install failure removes a pending secret or whether uninstall revokes the
dependent credential before deleting its encrypted delivery material.

For confidential OIDC clients, the implemented registration sequence is:

1. store the generated client secret under
   `rumahl.oidc.client-secret`;
2. persist only its digest in the OIDC client repository;
3. on replay, verify the active client's complete declaration-derived metadata
   and digest against the decrypted stored value;
4. return the same client and secret for runtime delivery.

The operation coordinator still needs to mark the OIDC step applied, deliver
the value through the runtime secret channel, and persist an acknowledgement
before applying the final retention/removal policy.

Public OIDC clients never create this secret.
