#!/bin/sh
# テスト用フェイクclaude CLI(異常終了する版)。
cat >/dev/null
echo "Error: not logged in. Run 'claude login'." >&2
exit 3
