# Native optional-provider credentials

This crate is the native credential authority for optional SonoVault, AudD and RapidAPI integrations. It does not enable providers, make network requests, read home-directory files or require an account to use CueRithm. A provider requiring a personal key receives it only after the user chooses that provider. UI setup remains a separate integration.

## Native API

Construct an `Arc<FileStateStore>` with `FileStateStore::open(explicit_app_data_directory)?`, then construct `CredentialService::new(NativeVault::new()?, state_store)?` once and share its clones. Both `Vault` and durable `StateStore` are required injections. Tests use an explicit memory state adapter; production has no memory-state default and no file/environment/plaintext key fallback. A future SQLite `StateStore` must preserve the same durable-intent, compare-and-set and exclusive-authority contracts.

- `status(provider)` returns a serializable `CredentialStatus`: provider/generation reference, presence, vault availability and verification enum. Locked/unavailable access has unknown presence rather than a misleading missing-key state.
- `save(&expected_reference, Secret::new(input)?)` compares the displayed generation, invalidates previous requests and writes to the native vault. Replacement resets verification. Clear the transient UI input after save; never put it in settings, presets, logs or job snapshots.
- `remove(provider)` unconditionally invalidates previous work before deleting the vault entry. Removing an absent entry succeeds. A queued save carrying the old reference cannot recreate it.
- `verify(&reference, native_callback)` runs a provider-specific check and records only a bounded verification enum for that generation. The callback receives the key and a revocation signal; it must not upload audio or spend recognition credits merely to test a key.
- `run(&reference, native_callback)` gives a trusted Rust provider operation temporary access to the key and the same revocation signal. It has no bridge/query counterpart. Returning, retaining or logging key material from a callback is forbidden; return only normal provider data with credentials removed. There is no public service method returning a saved key.

The presence/status DTO contains no secret, key prefix, header or raw vault error. `Secret` has no serialization or clone implementation; its debug output is redacted and its owned buffer is zeroized on drop. Low-level `Secret::expose` exists for trusted vault/provider adapters only. Vault errors map to bounded codes; credential-bearing byte payloads from native decoding errors are wiped before discard. Do not log native errors before this mapping.

## Concurrency and failure behavior

Vault operations serialize under one authority lock. Network callbacks run outside that lock, allowing replacement/removal to revoke in-flight work immediately. A callback must check revocation immediately before dispatch and integrate it with transport cancellation while waiting. The service rejects results after revocation, but cannot retract a request already accepted by a remote service. It deliberately does not implement HTTP clients, origin/redirect enforcement, quota scheduling or cancellation transports; those remain provider integration requirements.

Before touching the native vault, save/remove atomically persist a new generation and pending intent. Only native success followed by a durable acknowledgment clears that intent. If the process stops between either step, the next authority reads the pending marker and refuses to use any stored key, including a key that still exists after failed removal. The user must explicitly replace or remove it to settle the marker. A marker write failure prevents native mutation. Marker acknowledgment failure retains the pending intent; querying status never clears it. Failed removal is never reported as successful deletion.

Provider generation UUIDs persist and rotate on each mutation. A separate random authority-session UUID makes every pre-restart job reference stale, even after a successful settled save. Old references cannot save a deleted key again or acknowledge new verification. Verification itself is intentionally session-local and returns to Untested after restart.

`FileStateStore` stores only schema, provider IDs, generation UUIDs and save/remove intent in `credentials-state/state.json`, under the explicitly supplied existing application-data directory. It creates a private subdirectory and temporary file, flushes the staged JSON, atomically replaces the state file and syncs its directory on Unix. Windows flushes staged data before atomic replacement; actual filesystem/power-loss guarantees still require native acceptance. Unix modes are 0700 for the directory and 0600 for the JSON; Windows inherits the authorized application-data ACL. Symlink/reparse files and directories are rejected; corrupt, oversized or unknown-schema state is not silently reset. Keep this non-secret marker state with the application settings; never delete it as a cache or export it in presets. The file contains no key bytes, key hashes or key prefixes.

A lifetime OS file lock prevents a second live file-backed authority from interleaving native key mutations; it reports Busy. An additional file lock serializes state compare-and-set operations. GUI, Incoming and headless jobs must share the active authority. Alternative StateStore adapters must provide equivalent exclusive authority and durable commits. This crate does not detect out-of-band edits in the OS credential manager. Run synchronous native vault work away from the UI/async executor thread; the `run` callback must complete the authenticated operation before returning.

## Explicit platform backends

The crate follows the current keyring ecosystem's [recommendation to select concrete stores](https://docs.rs/keyring/latest/keyring/), using `keyring-core` directly rather than a process-global default builder:

- macOS: `apple-native-keyring-store` with `keychain`, using the user/login Keychain.
- Windows: `windows-native-keyring-store`, using generic Credential Manager entries with Local persistence rather than roaming Enterprise persistence.
- Linux: `dbus-secret-service-keyring-store` with encrypted `crypto-rust` communication and vendored native libraries. It still needs the desktop's Secret Service and unlocked user session; unavailable/locked service is surfaced without a substitute store.

Entries use a fixed nonempty CueRithm service namespace and fixed provider account identifiers. No enumeration of unrelated entries is performed. Unsupported OSes return Unavailable. No global default/mock store is selected by this implementation.

Official API references: [core](https://docs.rs/keyring-core/latest/keyring_core/), [macOS](https://docs.rs/apple-native-keyring-store/latest/apple_native_keyring_store/), [Windows](https://docs.rs/windows-native-keyring-store/latest/windows_native_keyring_store/), [Secret Service](https://docs.rs/dbus-secret-service-keyring-store/latest/dbus_secret_service_keyring_store/).

## Validation boundary

Tests use only an injected memory vault and synthetic keys, with either an explicit memory state adapter or the real file state implementation in disposable temporary directories. Restart fault tests cover intent-before-native-write, partial native failure, native success before acknowledgment, failed deletion, stale queued saves and exclusive authority. The macOS adapter compiles as part of these tests, but tests never construct it or call an OS store. Native save/read/delete, locked prompts, packaging, signing identity, Linux service availability and Windows persistence still require separate disposable native acceptance tests on each OS. No developer credentials or live provider calls are involved.
