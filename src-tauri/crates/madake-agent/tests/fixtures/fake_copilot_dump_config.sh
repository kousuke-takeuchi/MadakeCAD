#!/bin/sh
# テスト用フェイクcopilot CLI(--additional-mcp-config が指す一時ファイルの中身を
# 回答本文として返す版)。一時ファイルが実際に作られ、MadakeCADの内蔵MCPを
# 指していることの検証に使う。
config=""
while [ $# -gt 0 ]; do
  case "$1" in
    --additional-mcp-config)
      config=$(printf '%s' "$2" | sed 's/^@//')
      shift 2
      ;;
    *) shift ;;
  esac
done

if [ ! -f "$config" ]; then
  echo "MCP設定ファイルが存在しません: $config" >&2
  exit 9
fi

python3 -c '
import json, sys
text = open(sys.argv[1]).read()
print(json.dumps({"type": "assistant.message", "data": {"messageId": "m1", "content": text, "toolRequests": []}}, ensure_ascii=False))
print(json.dumps({"type": "result", "sessionId": "fake", "exitCode": 0, "usage": {"premiumRequests": 0.0}}))
' "$config"
