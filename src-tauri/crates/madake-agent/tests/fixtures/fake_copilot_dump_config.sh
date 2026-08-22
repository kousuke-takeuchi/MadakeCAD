#!/bin/sh
# テスト用フェイクcopilot CLI(--additional-mcp-config が指す一時ファイルの中身を返す版)。
# 一時ファイルが実際に作られ、MadakeCADの内蔵MCPを指していることの検証に使う。
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

python3 -c 'import json,sys; print(json.dumps({"type":"result","result":open(sys.argv[1]).read()}))' "$config"
