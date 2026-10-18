#!/bin/sh
# テスト用フェイクcopilot CLI(添付つきのターン用)。
# 本文に並んだ添付のパスが実在し、すべて`--add-dir`で渡されたフォルダの中にあることを確かめ、
# 「ok:<件数>:<フォルダ>」を回答本文として返す。
prompt=""
dirs=""
while [ $# -gt 0 ]; do
  case "$1" in
    -p|--prompt) prompt="$2"; shift 2 ;;
    --add-dir) dirs="$dirs
$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '%s' "$prompt" | DIRS="$dirs" python3 -c '
import json, os, sys
text = sys.stdin.read()
dirs = [d for d in os.environ["DIRS"].split("\n") if d]
paths = [l[2:] for l in text.split("\n") if l.startswith("- /")]
attach_dir = dirs[-1] if dirs else ""
ok = paths and all(os.path.isfile(p) and p.startswith(attach_dir + os.sep) for p in paths)
answer = "ok:%d:%s" % (len(paths), attach_dir) if ok else "ng:" + text
print(json.dumps({"type": "assistant.message", "data": {"messageId": "m1", "content": answer, "toolRequests": [], "outputTokens": 1}}, ensure_ascii=False))
print(json.dumps({"type": "result", "sessionId": "fake", "exitCode": 0, "usage": {"premiumRequests": 0.33}}))
'
