#!/bin/sh
# テスト用フェイクclaude CLI(2本のターンが**同時に**走ることを確かめる版)。
#
# プロンプトの `rendezvous:<dir>` で指定されたディレクトリへ自分の到着印を置き、
# 相手のターンが来るまで待ってから応答する。ターンが直列化されていると相手は
# 永久に来ないので待ちきれず "timeout" を返す(= 並行実行できていない印)。
# プロンプトに `hold` を含めると、相手を待ち合わせたあと少し長く居残る
# (どちらのターンが先に終わるかを決めたいテスト用)。
dir=""

if [ "$1" = "--version" ]; then
  echo "2.1.237 (Claude Code)"
  exit 0
fi

# プロンプトはstdin経由。読み捨てないと書き手側がEPIPEになる
prompt=$(cat)
dir=$(printf '%s' "$prompt" | sed -n 's/.*rendezvous:\([^ ]*\).*/\1/p')
result="$prompt"

if [ -n "$dir" ]; then
  : >"$dir/$$"
  i=0
  while [ "$(ls -1 "$dir" | wc -l | tr -d ' ')" -lt 2 ]; do
    i=$((i + 1))
    if [ "$i" -gt 100 ]; then
      result="timeout"
      break
    fi
    sleep 0.05
  done
fi

case "$prompt" in
*hold*) sleep 0.5 ;;
esac

printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"s"}\n' "$result"
