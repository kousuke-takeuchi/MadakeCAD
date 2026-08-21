#!/bin/sh
# テスト用フェイクclaude CLI(イベントにならない行だけを延々と吐く版)。
# thinking_deltaと未知タイプの行はAgentEventに変換されないため、tx.send()は
# 一度も呼ばれない。受信側がキャンセルで閉じてもsendの失敗では気付けず、
# 読み取りループがcloseを直接監視していないとCLIが走り続ける — その回帰テスト用。
cat >/dev/null
thinking='{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"むむむ"}}}'
unknown='{"type":"rate_limit_event","status":"ok"}'
while :; do
  echo "$thinking"
  echo "$unknown"
done
