#!/bin/sh
# テスト用フェイクclaude CLI(--append-system-prompt の中身を検査する版)。
# 規格知識は数KB・複数行あり、そのままresultへ載せるとJSONが壊れるため、
# 目印が入っているかどうかだけを1行のフラグ列として返す。
cat >/dev/null
prompt=""
model=""
prev=""
for a in "$@"; do
  case "$prev" in
    --append-system-prompt) prompt="$a" ;;
    --model) model="$a" ;;
  esac
  prev="$a"
done
flags=""
[ -n "$prompt" ] || flags="$flags no-system-prompt"
[ -z "$model" ] || flags="$flags model=$model"
case "$prompt" in *"JIS C 0617"*) flags="$flags standards" ;; esac
case "$prompt" in *"run_verification"*) flags="$flags verify-loop" ;; esac
case "$prompt" in *"アクティブシート: S1"*) flags="$flags drawing" ;; esac
case "$prompt" in *"社内ルール"*) flags="$flags user-knowledge" ;; esac
printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"s"}\n' "$flags"
