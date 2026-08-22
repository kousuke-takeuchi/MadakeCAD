#!/bin/sh
# テスト用フェイクcopilot CLI。実機(copilot 1.0.80)の`--output-format json`から
# 採取したJSONLを流す。本物はGitHub認証とAIクレジットが要るのでテストからは呼ばない。
#
# 実機と同じく、最終行の`result`では渡された`--session-id`(継続なら`--resume`)を
# そのまま返す。
dir=$(dirname "$0")

if [ "$1" = "--version" ]; then
  echo "GitHub Copilot CLI 1.0.80."
  exit 0
fi

session=""
while [ $# -gt 0 ]; do
  case "$1" in
    --session-id|--resume)
      session="$2"
      shift 2
      ;;
    *) shift ;;
  esac
done

sed "s/SESSION_ID/${session:-00000000-0000-0000-0000-000000000000}/" "$dir/copilot_stream.jsonl"
