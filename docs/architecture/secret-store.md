# Secret store

Status: engine-neutral core contract, encrypted SQLite adapter, recoverable
OIDC client-secret registration, journalled delivery acknowledgement, and
Buildroot-facing TPM/runtime-channel endpoints plus namespace-specific runtime
materialization implemented. Device provisioning remains deployment work.

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

`Tpm2UnsealKeyProvider` maps active and historical key IDs to TPM-sealed object
contexts and invokes a fixed absolute `tpm2_unseal` executable without a shell.
It accepts exactly 32 bytes, supports policy-session authorization without
placing a password in process arguments, zeroizes captured output, and fails
closed on any tool or length error. Buildroot provisioning must create the
device-bound objects, bind their policy to the intended measured-boot/update
state, protect the configuration, and define backup/recovery behavior.

## Lifecycle

Secret values are zeroized when their domain wrappers are dropped, and debug
output exposes only non-secret metadata. Removal by `InstallationId` is one
atomic store statement. The operation runner removes runtime material, revokes
the dependent OIDC client, and then deletes the encrypted value during
uninstall or install compensation. Each action is idempotent so an interrupted
removal can resume from its journal state.

For confidential OIDC clients, the implemented registration sequence is:

1. store the generated client secret under
   `rumahl.oidc.client-secret`;
2. persist only its digest in the OIDC client repository;
3. on replay, verify the active client's complete declaration-derived metadata
   and digest against the decrypted stored value;
4. return the same client and secret for runtime delivery.

`AppOperationRunner` now delivers the recovered value through the privileged
`RuntimeSecretDelivery` contract before marking the OIDC journal step applied.
If delivery fails or the process exits before that transition, recovery returns
the same registered client and encrypted secret and safely repeats delivery.
The runtime adapter must make a replay for the same operation, installation,
client, and value idempotent and reject a changed value. The journal's applied
transition is the durable delivery acknowledgement.

`UnixRuntimeSecretDelivery` is the concrete Buildroot-side client channel. It
authenticates the local supervisor UID using kernel Unix-socket peer
credentials before sending a length-prefixed request, uses bounded I/O waits,
zeroizes secret-bearing request buffers, and fails closed on replay conflicts
or malformed acknowledgements. `UnixRuntimeSecretServer` enforces the receiving
side, validates bounded domain fields, and delegates only authenticated
requests to `RuntimeSecretTarget`. `NamespaceRuntimeSecretTarget` binds each
delivery to its prepared installation namespace under the volatile runtime
root. It atomically publishes only the OIDC client ID and secret into the
container's read-only secret mount, records non-secret replay metadata outside
that mount, rejects untrusted paths or changed replays, and removes all runtime
material idempotently. Plaintext remains outside SQLite, logs, arguments, and
environment variables.

Update-time credential rotation and its rollback policy remain
operation-policy work.

Public OIDC clients never create this secret.
