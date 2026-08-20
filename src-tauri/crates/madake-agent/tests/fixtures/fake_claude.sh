#!/bin/sh
# テスト用フェイクclaude CLI。引数は無視してfixtureのstream-jsonをそのまま流す。
# 本物のclaudeバイナリをテストから呼ばないための代替。
dir=$(dirname "$0")

if [ "$1" = "--version" ]; then
  echo "2.1.237 (Claude Code)"
  exit 0
fi

# プロンプトはstdin経由で渡ってくる。読み捨てないと書き手側がEPIPEになる
cat >/dev/null

cat "$dir/stream_tooluse.jsonl"
