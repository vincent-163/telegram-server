# Protocol compatibility

This document is the source of truth for the behavior currently implemented by
this repository. It separates verified server behavior from deliberately
excluded external integrations.

## MTProto transports

| Transport | Status |
|---|---|
| Abridged | Implemented |
| Intermediate | Implemented |
| Padded intermediate | Implemented |
| Full | Implemented |
| Obfuscated transports | Implemented |

## Authorization key handshake

| Step | Status |
|---|---|
| `req_pq_multi` / `req_pq` | Implemented |
| `resPQ` and RSA fingerprints | Implemented |
| `req_DH_params` + RSA-padded `p_q_inner_data` | Implemented |
| `server_DH_params_ok` | Implemented |
| `set_client_DH_params` | Implemented |
| `dh_gen_ok` | Implemented |
| Persistent authorization keys | Implemented |

A Rust integration test runs all three handshake phases and compares the
client/server authorization keys and nonce hashes.

## RPC namespaces

Implemented namespaces and representative methods:

| Namespace | Implemented |
|---|---|
| `help` | `getConfig`, `getNearestDc` |
| `auth` | `sendCode`, `signIn`; `signUp` explicitly disabled |
| `users` | `getUsers`; `getFullUser` returns an explicit unsupported error |
| `account` | `updateProfile` |
| `messages` | `getDialogs`, `getHistory`, `sendMessage`, `editMessage`, `deleteMessages`, `readHistory`, `getChats`, `getFullChat`, `getState` |
| `updates` | `getState`, `getDifference` |
| `upload` | `saveFilePart`, `saveBigFilePart` |
| `contacts` | `getContacts`, `importContacts`, `resetSaved` |
| `langpack` | `getLanguages`, `getDifference`, `getLanguage` |
| containers | `invokeWithLayer`, `invokeWithoutUpdates`, `initConnection`, `msgs_ack`, `msg_container` |
| `ping` | `ping` |

Unsupported methods return an RPC error containing the generated TL method name
instead of silently succeeding.

## Deliberate boundaries

- Payments are not implemented.
- SMS/phone delivery is not implemented; login codes are configured locally.
- Phone calls are not implemented.
- Account registration is administrator-only.
- External Telegram CDN/config fetching is not used; the deployment serves a
  self-contained DC configuration.

## HTTP Bot API

| Method | Status |
|---|---|
| `getMe` | Implemented |
| `sendMessage` | Implemented |
| `sendPhoto` | Implemented |
| `sendDocument` | Implemented |
| `getUpdates` | Implemented |
| `getChat` | Implemented |
| `sendChatAction` | Implemented |
| `setWebhook` / `deleteWebhook` | Acknowledged no-op |
| `setMyCommands` / `deleteMyCommands` | Acknowledged no-op |

Files uploaded through the MTProto or Bot API paths are stored in SQLite and
served by `/file/bot<token>/<file_id>`.

## Server public key discovery

`GET /server-key` returns the MTProto fingerprint, both common PEM encodings,
the modulus/exponent, and the MTProto port. Clients must inject that public key
into their handshake key set and route the configured private DC address to the
server.
