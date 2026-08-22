#!/bin/sh
# テスト用フェイクcopilot CLI(stdoutへ延々と書き続ける版)。
# 受信側がキャンセルで閉じたあと、子プロセスをkillしないとパイプが詰まって
# child.wait()が返らなくなる — その回帰テスト用。
line='{"type":"assistant","text":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}'
while :; do
  echo "$line"
done
