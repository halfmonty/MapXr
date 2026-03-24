# ADB Wireless Debugging — Diagnosis Log

This document records the findings from debugging the ADB wireless connection in the
Android shell server flow (Epic 20, task 20.9). Read this before touching any ADB-related
code in the next session.

---

## Current state (as of 2026-03-23)

- **Pairing works**: SPAKE2 handshake completes, device UI shows `mapxr@mapxr` in Paired
  Devices, fingerprint is consistent across pairing and subsequent connection attempts.
- **Connection fails**: TLS handshake to the ADB connection port closes with
  `SSLHandshakeException: connection closed` every time.
- **Root cause confirmed**: The standard `adb pair` + `adb connect` from the host PC
  succeeds on the same device and port in the same session. This proves adbd is working
  correctly — the fault is specific to our app's pairing or key format.

---

## Device under test

Samsung Galaxy (Samsung-specific adbd behaviour noted throughout).
Android API 36. Wireless Debugging port changes each time Wireless Debugging is
toggled off/on. NSD (`_adb-tls-connect._tcp`) always shows two entries: one stale/dead
port (ECONNREFUSED) and one current port.

---

## What has been fixed

### 1. AndroidKeyStore → file-based RSA key

**Problem**: `AdbKey` originally used AndroidKeyStore. Samsung's KeyMint returns
`INCOMPATIBLE_PADDING_MODE` (logged by `keystore2`, pid 1358) when Conscrypt tries to
use the AndroidKeyStore RSA private key for TLS 1.3 client certificate signing. The error
occurs during `upgrade_keyblob_if_required_with` regardless of which padding modes are
declared during key generation. This is a Samsung KeyMint limitation, not a config issue.

**Fix applied**: `AdbKey` now stores the RSA 2048 keypair as files in
`context.filesDir`:
- `mapxr_adb.p8` — PKCS8 DER private key
- `mapxr_adb.crt` — DER X.509 self-signed certificate (BouncyCastle
  `JcaX509v3CertificateBuilder`, `SHA256withRSA`, 10-year validity, `CN=MapXr`)

All `AdbKey` methods now take `Context` as first parameter. AndroidKeyStore is gone.

### 2. Pairing TLS uses the same key as PeerInfo

**Problem**: The original code used an ephemeral in-memory RSA keypair for the pairing
TLS context, while PeerInfo contained the AndroidKeyStore key. Samsung's adbd silently
discards the trusted-key registration when the pairing TLS client certificate key does
not match the PeerInfo RSA key.

**Fix applied**: `AdbPairing.buildPairingTlsContext()` now presents `AdbKey.certificate(context)`
as the TLS client certificate — the same key that appears in PeerInfo.

### 3. NSD host resolution (Samsung-specific)

**Problem**: Samsung's adbd binds to the LAN interface (e.g. `192.168.1.191`), not
`127.0.0.1`. Connecting to the hardcoded loopback address reached a stale zombie adbd
that rejected the key.

**Fix applied**: `ShellServerManager.discoverAdbEndpoints()` resolves
`_adb-tls-connect._tcp` via NSD and uses `si.host?.hostAddress` from each resolved
service. Collects all results over a 2-second window (Samsung registers stale entries
alongside the current one). De-duplicates by port.

### 4. CNXN null terminator

**Problem**: Our CNXN payload was `"host::MapXr"` (no null terminator). Standard ADB
protocol uses null-terminated connection strings.

**Fix applied**: Changed to `"host::\u0000"` matching dadb's `CONNECT_PAYLOAD`.

### 5. `app_process` command format

**Problem**: The shell server start command used `-Djava.class.path=` which is not the
correct convention for `app_process`. The process was also not detached from adbd's
process group and would die when the ADB session closed.

**Fix applied**:
```kotlin
"(CLASSPATH=$DEX_REMOTE_PATH app_process /system/bin $SERVER_CLASS </dev/null >/dev/null 2>&1 &)"
```

---

## The remaining bug

### Symptom

After SPAKE2 pairing completes (confirmed by device UI showing `mapxr@mapxr` in Paired
Devices and matching fingerprint in logs), the TLS connection to the ADB connection port
is rejected:

```
ADB connect failed (192.168.1.191:33621): TLS rejected — key not trusted; re-pair: connection closed
```

The `SSLHandshakeException: connection closed` means adbd accepts the TCP connection,
TLS begins, then adbd closes the connection. No `system_server` SSL error appears in
logcat (unlike the earlier KeyMint issue, which did log server-side errors).

### Confirmed root cause

**The host PC running `adb pair 192.168.1.191:PAIRING_PORT CODE` followed by
`adb connect 192.168.1.191:PORT` succeeds on the same device and port in the same
session.**

This proves:
- adbd is running correctly on the connection port
- The pairing→connection flow works for standard clients
- The fault is in **our app's pairing or key format**, not in adbd or the network

### Likely cause

The most probable explanation is a mismatch between:

1. The RSAPublicKey struct (524 bytes) that **we compute** in `AdbKey.adbPublicKeyBytes()`
   and embed (base64-encoded) in PeerInfo during SPAKE2 pairing, and
2. The RSAPublicKey struct that **adbd computes** from our TLS client certificate's RSA
   public key when verifying an incoming connection.

adbd (BoringSSL) extracts the RSA modulus and exponent from our X.509 client cert,
independently computes the 524-byte Montgomery-form RSAPublicKey struct, base64-encodes
it, then looks for that string in `adb_keys`. If our PeerInfo base64 doesn't match adbd's
computation, the key is never found and TLS is rejected.

Our `adbPublicKeyBytes()` computation has been compared against dadb's
`convertRsaPublicKeyToAdbFormat()` and appears mathematically equivalent. However, a
subtle difference may exist that hasn't been caught by code review alone.

### Alternative cause (less likely)

SPAKE2 pairing completes but adbd does not write our key to `adb_keys` at all — for
example, if Samsung's adbd validates the PeerInfo contents before writing and rejects our
key for an undiscovered reason. The UI "Paired Devices" entry and the actual `adb_keys`
file update may be decoupled.

---

## Recommended next debugging steps

### Step 1 — Read `adb_keys` directly

Use the now-working host PC ADB connection to inspect what adbd actually stored:

```bash
adb -s 192.168.1.191:PORT shell "run-as com.mapxr.app cat /data/misc/adb/adb_keys"
# or if that fails:
adb -s 192.168.1.191:PORT shell "su -c 'cat /data/misc/adb/adb_keys'"
```

Compare the stored base64 key against what our app computed. The app's PeerInfo base64
can be read from the logcat if a `Log.d` is added to `buildPeerInfo()`.

### Step 2 — Add PeerInfo logging

In `AdbPairing.buildPeerInfo()`, log the first 64 chars of the base64 key string and
its total length. The ADB RSAPublicKey struct for a 2048-bit key should always be exactly
524 bytes → 700 base64 characters (with `=` padding).

```kotlin
Log.d(TAG, "PeerInfo key (first 64): ${keyField.take(64)}, total length: ${keyField.size}")
```

If the length is not 700 chars (before the space), the struct computation is wrong.

### Step 3 — Verify TLS cert is being sent

Add logging to the `X509KeyManager` in `AdbConnection.buildMutualTlsSocket()`:

```kotlin
override fun getCertificateChain(alias: String?): Array<X509Certificate>? {
    Log.d(TAG, "getCertificateChain called, alias=$alias")
    val cert = AdbKey.certificate(context) ?: run {
        Log.e(TAG, "certificate is null!")
        return null
    }
    Log.d(TAG, "presenting cert, subject=${cert.subjectDN}")
    return arrayOf(cert)
}
```

If `getCertificateChain` is never called, Conscrypt is not sending the client certificate
at all (TLS server not requesting it, or Conscrypt skipping it).

### Step 4 — If base64 lengths match but key still rejected

If step 1 confirms our base64 IS in `adb_keys` (or we can't read the file), the issue is
in how adbd computes the struct from the TLS cert. Try having the PC (with confirmed
working ADB) read the stored key:

```bash
adb shell cat /data/misc/adb/adb_keys
```

This shows exactly what adbd stored for the PC's key. Compare the format (length, content
prefix) against what our app logs.

---

## Testing procedure for a clean state

Do this before every fresh test run:

1. Wireless Debugging → remove all entries under "Paired devices"
2. Turn Wireless Debugging **off**
3. Phone Settings → Apps → mapxr → Storage → **Clear Data**
4. PC terminal: `adb kill-server`
5. Turn Wireless Debugging **on** — note the connection port shown on screen
6. Open the app, go through pairing (using a fresh pairing code from "Pair device with
   pairing code")
7. Watch logcat — **do not toggle Wireless Debugging** during the test

**Never toggle Wireless Debugging between pairing and connection test** — this changes
the port and invalidates the trusted key list.

---

## Key files

| File | Role |
|------|------|
| `gen/android/app/src/main/java/com/mapxr/app/AdbKey.kt` | RSA key storage (file-based) |
| `gen/android/app/src/main/java/com/mapxr/app/AdbPairing.kt` | SPAKE2 pairing flow |
| `gen/android/app/src/main/java/com/mapxr/app/AdbConnection.kt` | TLS ADB connection + ADB protocol |
| `gen/android/app/src/main/java/com/mapxr/app/ShellServerManager.kt` | Lifecycle, NSD discovery |
| `gen/android/app/src/main/java/com/mapxr/app/ShellServerPlugin.kt` | Tauri plugin entry point (calls pair + start) |
| `gen/android/shell-server/` | The DEX payload that runs as shell uid |
| `docs/spec/android-shell-server-spec.md` | Full spec for this epic |

---

## Final conclusions (2026-03-23)

**Decision: abandon direct ADB, replace Epic 20 with Shizuku (Epic 21).**

### What was definitively established

All of the following were verified correct and ruled out as the cause of failure:

- **SPAKE2 pairing handshake** — completes successfully every time; device shows `mapxr@mapxr`
  in Paired Devices.
- **RSA key generation and storage** — file-based PKCS8 + DER X.509, consistent across pairing
  and connection attempts.
- **RSAPublicKey struct computation** — 524 bytes, correct layout (`len=64`, `n0inv`, `n[64]`,
  `rr[64]`, `e=65537` LE). Manually decoded on PC with `base64 -d | wc -c` = 524. Matches PC
  `adbkey.pub` format exactly.
- **PeerInfo format** — 8192 bytes, type byte 0, then `base64(524 bytes) mapxr@mapxr\n`, null
  padded. Correct structure.
- **Base64 encoding** — standard alphabet (not URL-safe), no line wraps, correct padding (`=`).
  Manually verified decodable to exactly 524 bytes.
- **NSD endpoint discovery** — correctly resolves live connection port (confirmed against
  Wireless Debugging settings screen).
- **TLS cert presentation** — same RSA keypair used for pairing TLS client cert and PeerInfo;
  pairing TLS fix (issue #2 above) applied correctly.
- **CNXN null terminator** — applied (issue #4 above).

### Root cause

`adb logcat -s adbd` revealed:

```
adbd_auth: loading keys from /data/misc/adb/adb_keys
Invalid base64 key
Invalid base64 key
[server]: Handshake failed in SSL_accept/SSL_connect [CERTIFICATE_VERIFY_FAILED]
```

adbd is loading `adb_keys`, finding our entries, and rejecting them as "Invalid base64 key"
before the TLS handshake even reaches the point of requesting a client certificate. The PC's
key (which is stored via the same pairing flow and also 700 chars of standard base64 decoding
to 524 bytes) is accepted silently.

The exact reason adbd rejects our entries but accepts the PC's is unknown without root access
to read `adb_keys` directly and compare. All observable properties of our key are identical to
the PC's. The most likely explanation is a Samsung/Android 16 (API 36) modification to the
adbd pairing storage path that differs from AOSP in a way that cannot be debugged without
either root access or Samsung's adbd source.

### Why further debugging was abandoned

Further investigation would require one of:
- Root access to the test device (not available on a retail Samsung)
- Samsung's modified adbd source code (not public)
- An extensive trial-and-error approach with no clear convergence path

The time cost was not justified given that **Shizuku** provides the same capability
(`InputManager.injectInputEvent()` as shell uid) via a well-maintained library with a
documented API, no protocol reimplementation required, and an existing large user base. The
entire Epic 20 codebase is replaced by Epic 21. See `docs/plan/implementation-plan.md`.
