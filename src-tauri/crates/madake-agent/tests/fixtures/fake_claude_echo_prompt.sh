#!/bin/sh
# テスト用フェイクclaude CLI(stdinで受け取ったプロンプトをそのままresultとして返す版)。
# プロンプトがargvではなくstdin経由で渡っていることの検証に使う。
# 引数にプロンプトが混ざっていないことも確認する。
for arg in "$@"; do
  case "$arg" in
    -*) ;;
    stream-json|/*|mcp__madakecad__*) ;;
    *)
      echo "予期しないpositional引数: $arg" >&2
      exit 9
      ;;
  esac
done

prompt=$(cat)
# プロンプトにはJSONエスケープが要る文字を含めないこと(テスト側の取り決め)
printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"s"}\n' "$prompt"
