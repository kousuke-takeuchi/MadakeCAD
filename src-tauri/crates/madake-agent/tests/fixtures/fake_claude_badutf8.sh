#!/bin/sh
# テスト用フェイクclaude CLI(不正なUTF-8を吐く版)。行読みのIOエラー経路の検証用。
cat >/dev/null
printf '\377\376 not utf-8\n'
