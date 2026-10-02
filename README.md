# telegram-server

A standalone, single-node Telegram MTProto server and HTTP Bot API written in Rust.

The server is designed for private/LAN deployments where administrators create
accounts manually. It implements the MTProto authorization-key handshake, TCP
transports, encrypted request handling, SQLite persistence, core user/message
operations, and an HTTP Bot API.

## Features

- MTProto 2.0 authorization handshake with a persistent server RSA key.
- TCP transports: abridged, intermediate, padded intermediate, full, and obfuscated.
- SQLite persistence for users, auth keys, sessions, dialogs, messages, files, updates, and bot updates.
- Administrative account and bot creation with self-registration disabled.
- Core RPC groups: `ping`, `help`, `auth`, `users`, `account`, `messages`, `updates`, `upload`, `contacts`, `langpack`, and containers.
- HTTP Bot API methods: `getMe`, `sendMessage`, `sendPhoto`, `sendDocument`, `getUpdates`, `getChat`, `sendChatAction`, `setWebhook`, `deleteWebhook`, `setMyCommands`, and `deleteMyCommands`.
- Public endpoint exposing the configured server IP/port and MTProto RSA public key.
- No payments, SMS delivery, or phone calls. Accounts are provisioned by an administrator.

A method-level compatibility inventory is maintained in
[`docs/PROTOCOL.md`](docs/PROTOCOL.md).

## Quick start

```bash
cargo build --release
./target/release/telegram-server \
  --db ./telegram-server.db \
  --rsa-key ./telegram-server-rsa.key \
  --public-ip 192.168.37.27 \
  --mtproto-bind 0.0.0.0 \
  --mtproto-port 24443 \
  --bot-bind 0.0.0.0 \
  --bot-port 28081 \
  --login-code 12345
```

The RSA key file is generated on first start and must be kept private.
The matching public keys are exposed by the HTTP endpoint:

```bash
curl http://127.0.0.1:28081/server-key
```

Return fields:

- `fingerprint_u64`: hex fingerprint sent in `resPQ`.
- `public_key`: SPKI PEM (`-----BEGIN PUBLIC KEY-----`).
- `public_key_rsa`: PKCS#1 PEM (`-----BEGIN RSA PUBLIC KEY-----`).
- `mtproto_port`: TCP port clients should connect to.

## Administrator commands

Create a normal account:

```bash
./target/release/telegram-server \
  --db ./telegram-server.db \
  admin create-user --phone +8600000000000 \
  --first-name Alice --username alice --admin
```

Create a bot:

```bash
./target/release/telegram-server \
  --db ./telegram-server.db \
  admin create-bot --phone +8600000000001 \
  --username example_bot --bot-token 123456:replace-me
```

Generate an RSA key without starting the server:

```bash
./target/release/telegram-server admin genkey --out ./telegram-server-rsa.key
```

Private keys are written with mode `0600` where the platform supports POSIX permissions.

## HTTP Bot API

Local smoke test:

```bash
curl -s http://127.0.0.1:28081/healthz
curl -s -X POST http://127.0.0.1:28081/bot123456:replace-me/getMe
curl -s -X POST http://127.0.0.1:28081/bot123456:replace-me/sendMessage \
  -H 'content-type: application/json' \
  --data '{"chat_id":10000000001,"text":"hello"}'
curl -s -X POST http://127.0.0.1:28081/bot123456:replace-me/getUpdates \
  -H 'content-type: application/json' --data '{}'
```

Telegram Bot API clients can point to the HTTP base URL directly, for example:

```text
http://192.168.37.27:28081
```

## Deployment

A hardened systemd unit is provided in
[`deploy/telegram-server.service`](deploy/telegram-server.service).

```bash
sudo install -m 755 target/release/telegram-server /usr/local/bin/telegram-server
sudo install -m 644 deploy/telegram-server.service /etc/systemd/system/telegram-server.service
sudo systemctl daemon-reload
sudo systemctl enable --now telegram-server
sudo systemctl status telegram-server
```

Expected deployment ports:

- `24443/tcp`: MTProto
- `28081/tcp`: HTTP Bot API and `/server-key`

## Tests

```bash
cargo test
cargo fmt --check
```

The handshake integration test exercises `req_pq_multi`, RSA padding,
`req_DH_params`, the encrypted `server_DH_inner_data`, `set_client_DH_params`,
`dh_gen_ok`, and matching client/server authorization keys.

## License

MIT
