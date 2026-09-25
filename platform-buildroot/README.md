# rumahl Buildroot platform adapters

`rumahl-platform-buildroot` contains the Linux-facing adapters used by the
privileged rumahl platform service and its runtime supervisor. It does not
provision TPM objects or import app packages into an OCI engine; those remain
image- and device-lifecycle work.

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

The server binds without removing an existing path, uses owner-only `0600` by
default (the production supervisor explicitly selects `0660`), rejects
oversized, malformed, trailing, or domain-invalid fields, and maps the target's
applied, idempotent, conflicting, or rejected result back to the client. The
service manager must create a private, volatile parent directory and remove a
stale socket before process startup.

`NamespaceRuntimeSecretTarget` is the concrete supervisor implementation. It:

- run the server socket under a volatile root-owned directory such as `/run`;
- make delivery idempotent for operation ID, installation ID, client ID, and
  secret digest, returning conflict if the value changes;
- inject material only into the target installation's runtime namespace;
- never log or persist the plaintext, and zeroize its receive buffer;
- make removal idempotent before acknowledging it.

It writes the OIDC client ID and secret beneath
`<runtime-root>/<installation-id>/secrets/` and keeps replay metadata beside
that directory. Files are published atomically with bounded reads, symlink and
ownership checks, non-owner-write rejection, and zeroizing buffers. Docker
mounts only the installation's `secrets` directory read-only at
`/run/rumahl/secrets`; no engine socket or unrelated installation namespace is
exposed to the app. A changed operation, identity, client, or secret digest is
reported as a conflict, while an interrupted partial publication is completed
on replay.

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

### Modular Docker target

`DockerRuntimeTarget` is one concrete supervisor-side implementation. Docker
remains behind the engine-neutral `RuntimeControlTarget` contract, so an image
can replace it with containerd, Podman, or another OCI backend without changing
the platform service or operation journal. Package verification and image
import are separated again behind `DockerImageResolver`; the target accepts
only immutable SHA-256 image references.

The target invokes one fixed absolute Docker executable without a shell, clears
its environment, bounds command output and execution time, and derives names,
labels, secret-namespace mounts, and resource policy only from trusted
supervisor configuration. Containers are read-only, non-root, capability-free,
`no-new-privileges`, PID- and memory-limited, and attached only to the
supervisor-selected network. Complete runtime specs are fingerprinted for
conflict-safe replay.

The runtime supervisor is intended to run as its own service. Loss or restart
of Docker or this supervisor therefore makes runtime operations fail closed at
the Unix-socket boundary; it does not terminate the platform service. The
durable operation journal keeps ambiguous work recoverable, and the service
manager can restart the supervisor before reconciliation repeats the same
idempotent operation. Non-container platform functions do not depend on the
Docker process.

### Runtime supervisor process

The `rumahl-runtime-supervisor` binary hosts the runtime-control and
runtime-secret sockets in a separate, unprivileged process. Its configuration
is entirely explicit: fixed Docker executable, volatile runtime root,
staged-image root, supervisor-owned network, instance name, both socket paths,
and the platform service account whose UID is authenticated through
`SO_PEERCRED`. It probes both the Docker daemon and configured network before
accepting requests. Individual malformed requests, Docker command failures,
and timeouts are rejected without terminating the accept loops; an
unrecoverable listener failure exits so the service manager can replace the
process.

The production service creates the socket as `0660` in a `0750` runtime
directory owned by the dedicated `rumahl-runtime-control` group. Only
`rumahl-runtime` and `rumahl-platform` belong to that group; socket access is
still insufficient on its own because the server also requires the exact
configured platform UID. Library users keep the owner-only `0600` default.

`StagedDockerImageResolver` is the handoff from a future package importer. For
installation `<id>`, the importer must atomically create the supervisor-owned
file `<image-root>/<id>/image-reference` with mode `0600` or `0640`
and this exact bounded format:

```text
RDI1
runtime/server.oci
sha256:<64 lowercase hexadecimal digits>
```

The resolver rejects symlinked files, non-regular files, unexpected ownership,
group/world-writable paths, oversized or non-canonical metadata, mismatched
artifact paths, and mutable image references. The importer remains separately
replaceable and is responsible for signature verification and loading the
image before publishing this file.

`systemd/rumahl-runtime-supervisor.service` runs the process as the dedicated
`rumahl-runtime` account, grants only Docker-group access, creates protected
runtime/state directories, applies service sandboxing, and uses
`Restart=always`. `systemd/docker.service.d/10-rumahl-restart.conf` gives the
selected Docker engine the same bounded restart policy. A different OCI engine
ships its own target and service/drop-in while leaving the platform service
unchanged. The Buildroot image must create `rumahl-runtime`, `rumahl-platform`,
and the `rumahl-runtime-control` group, add only those two accounts to that
group, install both units, create the `rumahl-apps` network, and enable the
supervisor.

## Real-container end-to-end test

`tests/container_app_e2e.rs` exercises the full install and uninstall path with
a real container process and the standalone `rumahl-runtime-supervisor`
binary. It first verifies retryable failure before the immutable image is
staged, resumes the journaled installation, starts the prepared container only
during activation, and waits for an in-container readiness marker. It then
kills and replaces only the supervisor process before verifying ordered stop
and removal during uninstall; the platform runner, journal, and container stay
alive across that restart.

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

The fixture image is built from the pinned base and addressed by its resulting
immutable image ID. A Buildroot image may use this Docker target or replace it
through `RuntimeControlTarget`; its package-import implementation remains a
separate deployment choice.

## Browser shell deployment

The new [image external tree](image/README.md) and [platform service](../platform-service/README.md)
provide a separate, minimal browser milestone. The package installs platform and
SSR units, immutable frontend assets and an nginx HTTPS edge. Its design is
recorded in [the prototype migration note](../docs/architecture/buildroot-migration.md).
The runtime supervisor described above remains an independently integrated service.

### Runtime control timeouts

The runtime client waits up to 300 seconds for a control response by default.
This covers multi-command Docker operations: activation can execute 18 commands,
each bounded by the default 15-second Docker command timeout. The supervisor
keeps its separate five-second request I/O limit. Custom targets or Docker
timeouts require a matching explicit `UnixAppRuntimeProviderConfig::with_timeout`
budget. A client timeout does not cancel an operation already running in the
supervisor; recovery must recheck runtime state and use the existing replay-safe
operation journal.
