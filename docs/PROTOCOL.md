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

The server implements the transport, handshake and state layer for real, and
then answers the remaining MTProto method surface with protocol-valid default
objects so that official clients can boot and exercise their full UI.

### Hand-written handlers

These methods read and write actual server state:

| Namespace | Handlers |
|---|---|
| `help` | `getConfig`, `getNearestDc`, `getAppConfig`, `getSupport` |
| `auth` | `sendCode`, `signIn`; `signUp` is explicitly rejected |
| `users` | `getUsers`, `getFullUser` |
| `account` | `updateProfile`, `getNotifySettings`, `updateNotifySettings`, `getAuthorizations`, `resetAuthorization`, `resetAuthorizations` |
| `messages` | `getDialogs`, `getPinnedDialogs`, `getPeerDialogs`, `getHistory`, `getMessages`, `search`, `sendMessage`, `editMessage`, `deleteMessages`, `readHistory`, `getChats`, `getFullChat`, `getPeerSettings`, `getCommonChats`, `createChat` |
| `contacts` | `getContacts`, `importContacts`, `resetSaved`, `resolveUsername`, `search` |
| `updates` | `getState`, `getDifference` |
| `upload` | `saveFilePart`, `saveBigFilePart`, `getFile` |
| `channels` | `getChannels`, `getParticipants` |
| `photos` | `getUserPhotos` |
| `langpack` | `getLanguages`, `getDifference`, `getLanguage` |
| containers | `invokeWithLayer`, `invokeWithoutUpdates`, `initConnection`, `msgs_ack`, `msg_container` |
| `ping` | `ping` |

### Generated compatibility surface

`src/compat.rs` is generated from the `grammers-tl-types` schema by
`tools/gen_compat.py`. It maps 660 method constructor ids to a minimal,
protocol-valid TL literal of each method's declared return type, which lets a
stock client complete login and render its full interface even for features
this single-node server does not model.

Correctness of that table is enforced by `tests/compat.rs`: every entry is
parsed back with `grammers-tl-types`' own deserializer for the declared return
type, and the parse must consume the body exactly. Any entry that is not valid
TL fails the test.

### Deliberate boundaries

Six namespaces are *not* stubbed, so clients receive a normal RPC error for
them rather than a plausible-looking empty success:

- `payments`, `premium`, `fragment` — they move real money.
- `phone`, `smsjobs` — they need telephony infrastructure.
- `aicompose` — needs a hosted model.

In addition:

- SMS/phone delivery is not implemented; login codes are configured locally
  with `--login-code`.
- Phone calls are not implemented.
- Account registration is administrator-only; `auth.signUp` is rejected.
- QR-code and passkey login return an explicit unsupported-method error;
  phone-code login uses the administrator-configured local login code.
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
