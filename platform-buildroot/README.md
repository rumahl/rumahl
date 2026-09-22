# rumahl Buildroot platform adapters

`rumahl-platform-buildroot` contains the Linux-facing adapters used by the
privileged rumahl platform service and its runtime supervisor. It does not
provision a TPM object or implement an OCI engine; those remain image- and
device-lifecycle work.

## TPM-sealed secret-store root keys

`Tpm2UnsealKeyProvider` implements `SecretEncryptionKeyProvider` by invoking one
fixed, absolute `tpm2_unseal` executable path without a shell. Each configured
key ID maps to an absolute TPM object-context path. The active key encrypts new
records while older configured IDs remain available for decryption and
rotation.

The provider accepts exactly 32 bytes from standard output, zeroizes that
buffer after constructing the AES-256 key, clears the child environment, does
not provide standard input, and discards diagnostic output so it cannot enter
platform logs. Output capture is capped at 33 bytes and a configurable timeout
terminates a stuck tool. A non-zero exit, missing key ID, timeout, or wrong
output length fails closed. Authorization passwords are intentionally
unsupported because command arguments are observable. A pre-established TPM
policy session can instead be passed as an absolute session path.

Buildroot provisioning must:

- create a keyed-hash sealing object whose attributes include `fixedtpm` and
  `fixedparent`;
- bind the object to the intended PCR/update policy when measured-boot binding
  is required;
- protect the serialized object, policy-session files, executable, and service
  configuration as root-owned platform state;
- configure old sealed objects while encrypted records still reference their
  key IDs;
- define recovery and PCR resealing before shipping an update that changes the
  measured boot state.

The TPM tool returns the unsealed value in clear to the platform process, as
documented by
[`tpm2_unseal`](https://github.com/tpm2-software/tpm2-tools/blob/master/man/tpm2_unseal.1.md).
Linux trusted-key documentation describes the device trust sources and PCR
binding model in more detail:
[Trusted and Encrypted Keys](https://docs.kernel.org/security/keys/trusted-encrypted.html).

## Runtime secret channel

`UnixRuntimeSecretDelivery` sends OIDC runtime material to a local supervisor
over an absolute pathname Unix stream socket. `UnixRuntimeSecretServer`
provides the receiving side and delegates validated requests to a
`RuntimeSecretTarget` implemented by the supervisor. Both endpoints
authenticate the peer UID with Linux `SO_PEERCRED` (and `getpeereid` in macOS
host tests). They apply read/write deadlines and use a bounded,
length-prefixed binary protocol rather than command arguments, environment
variables, or durable files. Buffers holding delivered secrets are zeroized on
drop.

Protocol `RSH1` request layout:

```text
magic[4] | operation:u8 | field_count:u8 |
repeated(field_length:u32-be | field_bytes)
```

Operation `1` delivers fields in this order: operation ID, installation ID,
app ID, publisher ID, OIDC client ID, base64url client secret. Operation `2`
removes runtime material and contains operation ID and installation ID.

The supervisor replies with `RSH1` plus one status byte: `0` acknowledges the
operation, `1` reports a conflicting replay, and `2` rejects it. Unknown or
truncated responses fail closed.

The server binds without removing an existing path, changes the socket mode to
`0600`, rejects oversized, malformed, trailing, or domain-invalid fields, and
maps the target's applied, idempotent, conflicting, or rejected result back to
the client. The service manager must create a private, volatile parent
directory and remove a stale socket before process startup.

The supervisor's `RuntimeSecretTarget` must additionally:

- run the server socket under a volatile root-owned directory such as `/run`;
- make delivery idempotent for operation ID, installation ID, client ID, and
  secret digest, returning conflict if the value changes;
- inject material only into the target installation's runtime namespace;
- never log or persist the plaintext, and zeroize its receive buffer;
- make removal idempotent before acknowledging it.

Linux documents that `SO_PEERCRED` returns credentials captured for the peer of
a connected Unix socket in [`unix(7)`](https://man7.org/linux/man-pages/man7/unix.7.html).

## Runtime control channel

`UnixAppRuntimeProvider` is the concrete Buildroot implementation of the core
`AppRuntimeProvider` contract. It connects to `UnixRuntimeControlServer` over
an absolute-path Unix stream socket. Both sides authenticate the peer UID,
enforce read/write deadlines, bound every field and the complete request, and
reject malformed or logically impossible responses.

Protocol `RRP1` separates runtime preparation from activation:

```text
request  = magic[4] | operation:u8 | field_count:u8 |
           repeated(field_length:u32-be | field_bytes)
response = magic[4] | status:u8 | state:u8 | changed:u8
```

Operations are prepare, activate, state, deactivate, and remove. Prepare and
activate carry the validated installation ID, app ID, publisher ID, version,
runtime kind, and ordered entrypoint triples. Package paths remain relative;
host paths, container IDs, arbitrary Docker options, environment variables,
and secrets are not part of the protocol. Other operations carry only the
installation ID.

The supervisor target receives a typed `RuntimeInstallationSpec`. It must:

- derive all concrete package and namespace paths from trusted platform
  configuration and the installation ID;
- compare the complete specification on prepare and activate replays;
- create the namespace without executing app code during prepare;
- start only a matching prepared runtime during activation;
- report `Absent`, `Prepared`, or `Active` from the actual runtime engine;
- make stop and remove idempotent, while rejecting removal of an active
  runtime;
- translate `ContainerArtifact` package paths into the image's selected OCI
  engine without exposing that engine or its socket to the app.

The older prototype's supervisor exposed Docker-shaped app metadata and
combined creation with startup. This boundary keeps its useful lifecycle
separation and stable per-app supervision model, while deliberately excluding
Docker API access, host volume strings, caller-selected ports, and plaintext
environment credentials.

## Real-container end-to-end test

`tests/container_app_e2e.rs` exercises the full install and uninstall path with
a real container process. It connects `AppOperationRunner` through
`UnixAppRuntimeProvider` and `UnixRuntimeControlServer` to a deliberately
test-local Docker target. The target prepares a stopped container, starts it
only during runtime activation, waits for an in-container readiness marker,
then verifies ordered stop and removal during uninstall.

The fixture runs read-only, without networking or Linux capabilities, with
`no-new-privileges`, a PID and memory limit, an unprivileged UID, and only a
small volatile `/tmp`. The CI workflow pins the multi-architecture BusyBox
image by digest. The test is ignored in the normal suite because it requires a
Linux container daemon and an explicitly pre-pulled image:

```text
RUMAHL_CONTAINER_E2E_IMAGE=busybox@sha256:<digest> \
cargo test -p rumahl-platform-buildroot --test container_app_e2e \
  -- --ignored --exact installs_runs_and_uninstalls_real_container_app
```

The Docker target is test infrastructure, not the production OCI target. This
keeps the end-to-end lifecycle executable while the Buildroot image's final
OCI engine and package-import mechanism remain an explicit deployment choice.
