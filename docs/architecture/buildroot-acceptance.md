# QEMU browser milestone acceptance — 2026-09-24

The separate build VM returned `return-to-codex.tar.gz`, containing source and
harness patches, a source manifest, image hashes and build/VM logs. The source
patches and the earlier host-user-lookup test correction were reviewed and
integrated here. All 342 files in the returned source manifest matched the
integrated working tree before this documentation update. The source snapshot
includes uncommitted implementation changes over Git base
`ca2c33522349d737e47f4ed2aa69080a0a443fc6`; that commit alone does not reproduce it.

The build and VM were run on the external host, not repeated in this editor
workspace. Image hashes below identify the reported artifacts; the image files
were not returned for independent hashing. Raw logs and one-off harness/debug
scripts stay outside the product source tree.

## Integrated corrections

- Enable ICU in the QEMU defconfig. The returned guest diagnostic shows
  `ReferenceError: Intl is not defined` when importing the SSR bundle without
  ICU. Pinned Buildroot's Node package selects `--with-intl=system-icu` when ICU
  is enabled. The rebuilt image passed authenticated SSR requests for both users.
- Serialize the five Rust tests that create and execute helper programs, using
  a shared test-only mutex. The external host reported intermittent `ETXTBSY`
  failures under parallel execution and 12 passing repetitions after the fix.
  Runtime process handling and timeout assertions remain unchanged.
- The supervisor's host-dependent negative user-lookup test accepts both
  absence and lookup failure as rejection. It still requires a successful root
  lookup and rejects an unexpected successful lookup of the missing user.

The harness-only archive extraction correction and optional build-job override
are not product changes. The VM debug scripts are not installed into the image.

## Returned validation evidence

Host: Ubuntu 24.04 x86-64, glibc 2.39, 16 vCPUs, 21 GiB RAM. QEMU used TCG,
2 guest vCPUs and 2 GiB RAM, with a read-only system disk and separate writable
ext4 data disk. Buildroot is pinned to 2026.08; guest Node is 22.23.2 with ICU.

The returned logs record successful Rust format, Clippy and workspace tests,
frontend typecheck/lint/tests/build, the process-level HTTPS test, target
compilation and complete image creation. One existing Docker integration test
remains ignored.

`vm-acceptance.out` and `vm-acceptance.json` record 18 successful checks across
four boots separated by hard power-offs:

- HTTPS login and recovery reachable; anonymous shell requests redirect and
  anonymous snapshots return 401; a foreign login Origin returns 403.
- Alice and Bob log in independently, receive cookies with the expected flags,
  render the shell and receive snapshots with their respective display names.
- Both sessions survive a VM restart.
- Alice's old cookie returns 401 after logout and another restart; Bob's cookie
  continues to return 200. The test retains the old cookie to check server-side
  revocation rather than merely deleting it on the client.

The host's port 8443 was occupied. The acceptance client used host port 18443
with `curl --connect-to`; the logical HTTPS origin remained
`https://rumahl.home.arpa:8443`, with certificate verification enabled. The
shipped QEMU runner still binds host port 8443.

## Artifact identity

| Artifact | SHA-256 / ID |
| --- | --- |
| Returned source archive | `0b232eb30f1c0e1d1d7e3ff156398453bc6473a3ca36c37204ef555b33982f29` |
| Kernel `bzImage` | `b23bd0ae3483f4a7444249d03cc35da5ff82d309ce80ea81a25485cb88f0c0b0` |
| System `rootfs.ext2` | `e76d2cfbd82ed296aaca0d0d692db161b984718b82fc308a1d90396fef24d72b` |
| Shell build | `shell-98c99f229b12e974240e1e85` |

## Remaining acceptance work

There was no real-browser test: the VM checks use a TLS-verifying HTTP client,
so browser hydration, interactions and browser-enforced cookie/CSP behavior
still need acceptance. Renderer failure was tested at host-process level, not
inside the VM. The reported image and VM successes do not establish hardware,
installer, signed-update, A/B rollback or TPM acceptance.
