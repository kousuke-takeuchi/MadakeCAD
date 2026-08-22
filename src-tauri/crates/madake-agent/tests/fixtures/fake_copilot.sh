#!/bin/sh
# テスト用フェイクcopilot CLI。引数は無視してfixtureのJSONLをそのまま流す。
# 本物のcopilotバイナリ(要GitHub認証)をテストから呼ばないための代替。
dir=$(dirname "$0")

if [ "$1" = "--version" ]; then
  echo "GitHub Copilot CLI 1.0.80."
  exit 0
fi

cat "$dir/copilot_stream.jsonl"
