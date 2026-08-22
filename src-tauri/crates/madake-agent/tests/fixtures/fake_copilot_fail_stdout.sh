#!/bin/sh
# テスト用フェイクcopilot CLI(理由をstdoutの素のテキストで出して異常終了する版)。
# 実物のCLIはAIクレジット切れなどの理由をJSONLでない行で出すことがある。
echo "You have run out of AI credits for this month."
exit 1
