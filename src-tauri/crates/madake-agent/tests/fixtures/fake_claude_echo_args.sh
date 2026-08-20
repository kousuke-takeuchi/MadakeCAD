#!/bin/sh
# テスト用フェイクclaude CLI(--model / --append-system-prompt の値をresultへ反映する版)。
# マネージャからCLIへモデル・図面コンテキストが渡っていることの検証に使う。
cat >/dev/null
seen=""
prev=""
for a in "$@"; do
  case "$prev" in
    --model|--append-system-prompt) seen="$seen $prev=$a" ;;
  esac
  prev="$a"
done
printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"s"}\n' "$seen"
