#!/bin/sh
# テスト用フェイクclaude CLI(添付つきのターン用)。
# `--input-format stream-json`が付いていることを確かめ、stdinの1行(ユーザーメッセージ)を読んで、
# 内容ブロックの種類と本文を「image,document|本文」の形でresultとして返す。
has_input_format=0
prev=""
for arg in "$@"; do
  if [ "$prev" = "--input-format" ] && [ "$arg" = "stream-json" ]; then has_input_format=1; fi
  prev="$arg"
done
if [ "$has_input_format" != "1" ]; then
  echo "--input-format stream-json がありません" >&2
  exit 9
fi
python3 -c '
import json, sys
msg = json.loads(sys.stdin.readline())
content = msg["message"]["content"]
kinds = ",".join(b["type"] for b in content if b["type"] != "text")
text = [b["text"] for b in content if b["type"] == "text"][0]
result = kinds + "|" + text if msg["type"] == "user" and msg["message"]["role"] == "user" else "bad"
print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": result, "session_id": "s"}, ensure_ascii=False))
'
