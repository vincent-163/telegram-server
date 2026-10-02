#!/bin/sh
set -eu

base="${1:-http://127.0.0.1:28081}"
token="${2:-123456:replace-me}"

health="$(curl -fsS "$base/healthz")"
[ "$health" = "ok" ]

curl -fsS "$base/server-key" >/dev/null

get_me="$(curl -fsS -X POST "$base/bot$token/getMe" -H 'content-type: application/json' --data '{}')"
case "$get_me" in
  *'"ok":true'*) ;;
  *) echo "$get_me"; exit 1 ;;
esac

sent="$(curl -fsS -X POST "$base/bot$token/sendMessage" -H 'content-type: application/json' \
  --data '{"chat_id":10000000001,"text":"smoke"}')"
case "$sent" in
  *'"ok":true'*) ;;
  *) echo "$sent"; exit 1 ;;
esac

updates="$(curl -fsS -X POST "$base/bot$token/getUpdates" -H 'content-type: application/json' --data '{}')"
case "$updates" in
  *'"ok":true'*) ;;
  *) echo "$updates"; exit 1 ;;
esac

echo "smoke ok"
