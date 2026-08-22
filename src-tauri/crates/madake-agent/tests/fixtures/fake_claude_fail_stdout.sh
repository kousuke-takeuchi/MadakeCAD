#!/bin/sh
# テスト用フェイクclaude CLI(理由をstdoutの素のテキストで出して異常終了する版)。
# 実物のclaude CLIは利用上限などの理由をstream-jsonでない行でstdoutへ出すことがある。
cat >/dev/null
echo "You've hit your weekly limit. resets 1am (Asia/Tokyo)"
exit 1
