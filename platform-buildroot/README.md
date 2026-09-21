# rumahl Buildroot platform adapters

`rumahl-platform-buildroot` contains the Linux-facing security adapters used by
the privileged rumahl platform service. It does not provision a TPM object or
run the container supervisor; those remain image- and device-lifecycle work.

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
over an absolute pathname Unix stream socket. Before writing any request, the
client authenticates the connected peer UID with Linux `SO_PEERCRED` (and
`getpeereid` in macOS host tests). It applies read/write deadlines and uses a
length-prefixed binary protocol rather than command arguments, environment
variables, or durable files. The request buffer holding a delivered secret is
zeroized on drop.

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

The supervisor must additionally:

- create the socket under a volatile root-owned directory such as `/run` and
  restrict its filesystem mode;
- authenticate the connecting platform process from kernel peer credentials;
- make delivery idempotent for operation ID, installation ID, client ID, and
  secret digest, returning conflict if the value changes;
- inject material only into the target installation's runtime namespace;
- never log or persist the plaintext, and zeroize its receive buffer;
- make removal idempotent before acknowledging it.

Linux documents that `SO_PEERCRED` returns credentials captured for the peer of
a connected Unix socket in [`unix(7)`](https://man7.org/linux/man-pages/man7/unix.7.html).
