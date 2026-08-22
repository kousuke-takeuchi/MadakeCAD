#!/bin/sh
# テスト用フェイクcopilot CLI(-p で受け取ったプロンプトを回答本文として返す版)。
# Copilot CLIにはシステムプロンプト用のフラグが無く、システム指示をプロンプトへ
# 前置している — その合成結果を検証するために使う。
#
# 実機と同じく、回答本文は`assistant.message`に載り、最終行の`result`には載らない。
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
printf '%s' "$prompt" | python3 -c '
import json, sys
text = sys.stdin.read()
print(json.dumps({"type": "assistant.message", "data": {"messageId": "m1", "content": text, "toolRequests": [], "outputTokens": 1}}, ensure_ascii=False))
print(json.dumps({"type": "result", "sessionId": "fake", "exitCode": 0, "usage": {"premiumRequests": 0.33}}))
'
