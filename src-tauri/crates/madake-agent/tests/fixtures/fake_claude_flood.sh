#!/bin/sh
# テスト用フェイクclaude CLI(stdoutへ延々と書き続ける版)。
# 受信側がキャンセルで閉じたあと、子プロセスをkillしないとパイプが詰まって
# child.wait()が返らなくなる — その回帰テスト用。
cat >/dev/null
line='{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}}}'
while :; do
  echo "$line"
done
