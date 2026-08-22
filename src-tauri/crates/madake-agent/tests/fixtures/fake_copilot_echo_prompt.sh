#!/bin/sh
# テスト用フェイクcopilot CLI(-p で受け取ったプロンプトをそのままresultとして返す版)。
# Copilot CLIにはシステムプロンプト用のフラグが無く、システム指示をプロンプトへ
# 前置している — その合成結果を検証するために使う。
prompt=""
while [ $# -gt 0 ]; do
  case "$1" in
    -p|--prompt)
      prompt="$2"
      shift 2
      ;;
    *) shift ;;
  esac
done

# JSONとして安全に出すためにpythonでエンコードする(改行・引用符を含むため)
printf '%s' "$prompt" | python3 -c 'import json,sys; print(json.dumps({"type":"result","result":sys.stdin.read()}))'
