# 自動化・API

[English (canonical)](10-automation-api.md)

UIでできることはすべてスクリプト化できる。3つの入口が同じCommandエンジンを共有するため、自動編集もundo履歴に乗り、UIへ即時反映される。

## 利用できる機能

### MCPサーバー(AIクライアント向け)
`127.0.0.1:9310/mcp`(Streamable HTTP、`MADAKE_MCP_PORT`で変更可)。ツール:
`get_project`・`list_symbols`・`place_symbol`・`draw_wire`・`execute_commands`(Command全スキーマ公開)・`get_netlist`・`run_verification`・`simulate_op`・`search_parts`・`upsert_part`・`delete_part`・`import_kicad`・`export_svg`・`export_pdf`・`export_bom`・`export_wire_list`・`undo`・`redo`。
専用ツールが無い編集は`execute_commands`から実行する。M2で増えたコマンドもここに含まれる: `set_revisions`(改訂欄)、`renumber_wires`(`mode: append | renumber`・`sheet_id`任意・`start`)と`set_wire_numbers`(線番)、ハーネス境界は`add_entity`で`harness`エンティティを追加する。
Claude Codeはリポジトリの`.mcp.json`で自動接続。

### Link API(REST+SSE、外部ツール向け)
`127.0.0.1:9310/api/v1` — CLI・ブラウザでのUI検証・(将来)FreeCADアドオンが使う素のJSON REST:
- 読み: `/project`・`/symbols`・`/netlist`・`/verify`・`/parts`・`/wire-parts`
- 書き: `/commands`(Command配列)・`/undo`・`/redo`・`/save`・`/load`・`/import/kicad`・`/simulate/op`・`/export/{svg,pdf,bom,wire-list}`・部品CRUD
- ライブ更新: `GET /events`(SSEパッチストリーム)
- ローカルオリジンガード(外部Webオリジンからのアクセスを拒否)

### madake CLI
Link APIの薄いターミナルクライアント: `status`・`project`・`netlist`・`verify`・`sim`・`parts`・`export`・`save`/`open`(`.kicad_sch`対応)・`renumber`(線番採番: `--sheet`・`--mode append|renumber`・`--start`)・`exec`(JSONのCommand配列)・`undo`/`redo`。`--json`で機械可読出力。

## 計画中

- M5アドオンが必要とするFreeCAD向けエンドポイント(例: 3Dモデル有り部品のフィルタ)
- OSS公開に向けたAPIの安定性・バージョニング方針(M6)
