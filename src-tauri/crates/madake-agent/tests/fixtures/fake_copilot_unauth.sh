#!/bin/sh
# テスト用フェイクcopilot CLI(GitHub未認証の版)。
# 実機のcopilot 1.0.80が未認証で出す文面をそのまま再現している。
cat >&2 <<'EOF'
Error: No authentication information found.

Copilot can be authenticated with GitHub using an OAuth Token or a Fine-Grained Personal Access Token.

To authenticate, you can use any of the following methods:
  • Start 'copilot' and run the '/login' command
  • Set the COPILOT_GITHUB_TOKEN, GH_TOKEN, or GITHUB_TOKEN environment variable
  • Run 'gh auth login' to authenticate with the GitHub CLI
EOF
exit 1
