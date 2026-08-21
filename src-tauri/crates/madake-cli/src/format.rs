//! 人間向け出力の整形。全て純粋関数なのでHTTP無しでテストできる。

use serde_json::Value;

use crate::client::ExportKind;

/// 列の区切り。
const GUTTER: &str = "  ";

/// 端末表示幅 (全角=2)。表の桁揃えに使う。
pub fn disp_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    let cp = c as u32;
    let wide = matches!(cp,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE10..=0xFE19
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD);
    if wide {
        2
    } else {
        1
    }
}

/// 表示幅がwidthになるまで右側を空白で埋める。
pub fn pad(s: &str, width: usize) -> String {
    let w = disp_width(s);
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

/// 列幅を揃えた素朴なテキスト表。
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| disp_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(disp_width(cell));
        }
    }
    let render = |cells: &[String]| -> String {
        let line = (0..cols)
            .map(|i| {
                let cell = cells.get(i).map(String::as_str).unwrap_or("");
                pad(cell, widths[i])
            })
            .collect::<Vec<_>>()
            .join(GUTTER);
        line.trim_end().to_string()
    };
    let header: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let mut out = vec![render(&header)];
    out.extend(rows.iter().map(|r| render(r)));
    out.join("\n")
}

fn text(v: &Value, key: &str) -> String {
    match &v[key] {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn yes_no(v: &Value, key: &str) -> &'static str {
    if v[key].as_bool().unwrap_or(false) {
        "可"
    } else {
        "不可"
    }
}

/// シートのエンティティ種別ごとの件数 (symbol, wire, junction, net_label, text)。
fn entity_counts(sheet: &Value) -> [usize; 5] {
    let mut counts = [0usize; 5];
    if let Some(map) = sheet["entities"].as_object() {
        for entity in map.values() {
            let idx = match entity["kind"].as_str().unwrap_or("") {
                "symbol" => 0,
                "wire" => 1,
                "junction" => 2,
                "net_label" => 3,
                "text" => 4,
                _ => continue,
            };
            counts[idx] += 1;
        }
    }
    counts
}

fn sheets(snapshot: &Value) -> &[Value] {
    snapshot["project"]["sheets"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// `madake status`
pub fn status(health: &Value, snapshot: &Value, port: u16) -> String {
    let sheets = sheets(snapshot);
    let names: Vec<String> = sheets.iter().map(|s| text(s, "name")).collect();
    let entities: usize = sheets
        .iter()
        .map(|s| s["entities"].as_object().map(|m| m.len()).unwrap_or(0))
        .sum();
    let api = format!(
        "{} v{}",
        health["name"].as_str().unwrap_or("Link API"),
        health["version"].as_u64().unwrap_or(1)
    );
    let rows = [
        (
            "接続",
            format!("{} ({})", crate::client::base_url(port), api),
        ),
        ("プロジェクト", text(&snapshot["project"], "name")),
        ("リビジョン", text(snapshot, "revision")),
        (
            "シート",
            format!("{} 枚 ({})", sheets.len(), names.join(", ")),
        ),
        ("エンティティ", entities.to_string()),
        (
            "undo / redo",
            format!(
                "{} / {}",
                yes_no(snapshot, "can_undo"),
                yes_no(snapshot, "can_redo")
            ),
        ),
    ];
    let label_width = rows.iter().map(|(l, _)| disp_width(l)).max().unwrap_or(0);
    rows.iter()
        .map(|(l, v)| format!("{} : {}", pad(l, label_width), v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `madake project`
pub fn project(snapshot: &Value) -> String {
    let proj = &snapshot["project"];
    let mut out = vec![
        format!(
            "プロジェクト: {} (format_version {}, revision {})",
            text(proj, "name"),
            text(proj, "format_version"),
            text(snapshot, "revision")
        ),
        String::new(),
    ];

    let rows: Vec<Vec<String>> = sheets(snapshot)
        .iter()
        .map(|s| {
            let c = entity_counts(s);
            vec![
                text(s, "id"),
                text(s, "name"),
                text(s, "size"),
                text(s, "orientation"),
                c[0].to_string(),
                c[1].to_string(),
                c[2].to_string(),
                c[3].to_string(),
                c[4].to_string(),
            ]
        })
        .collect();
    out.push(table(
        &[
            "シートID",
            "名前",
            "用紙",
            "向き",
            "シンボル",
            "配線",
            "接続点",
            "ラベル",
            "テキスト",
        ],
        &rows,
    ));

    let parts = proj["wire_parts"].as_array().cloned().unwrap_or_default();
    if !parts.is_empty() {
        out.push(String::new());
        out.push(format!("電線品番: {} 件", parts.len()));
        let rows: Vec<Vec<String>> = parts
            .iter()
            .map(|p| {
                vec![
                    text(p, "part_no"),
                    text(p, "color"),
                    format!("{}sq", text(p, "sq")),
                    text(p, "note"),
                ]
            })
            .collect();
        out.push(table(&["品番", "色", "線径", "備考"], &rows));
    }
    out.join("\n")
}

/// `madake netlist`
/// `madake verify`
pub fn diagnostics(diags: &Value) -> String {
    let diags = diags.as_array().cloned().unwrap_or_default();
    if diags.is_empty() {
        return "問題は見つかりませんでした。".to_string();
    }
    let mark = |sev: &str| match sev {
        "error" => "✗",
        "warning" => "⚠",
        _ => "ℹ",
    };
    let mut errors = 0;
    let mut warnings = 0;
    let lines: Vec<String> = diags
        .iter()
        .map(|d| {
            let sev = text(d, "severity");
            match sev.as_str() {
                "error" => errors += 1,
                "warning" => warnings += 1,
                _ => {}
            }
            format!("{} [{}] {}", mark(&sev), text(d, "code"), text(d, "message"))
        })
        .collect();
    format!(
        "検証結果: エラー {errors} / 警告 {warnings} / 情報 {}\n\n{}",
        diags.len() - errors - warnings,
        lines.join("\n")
    )
}

pub fn netlist(nets: &Value) -> String {
    let nets = nets.as_array().cloned().unwrap_or_default();
    if nets.is_empty() {
        return "ネットがありません。".to_string();
    }
    let rows: Vec<Vec<String>> = nets
        .iter()
        .map(|net| {
            let pins = net["pins"].as_array().cloned().unwrap_or_default();
            let refs: Vec<String> = pins
                .iter()
                .map(|p| format!("{}:{}", text(p, "reference"), text(p, "pin")))
                .collect();
            vec![
                text(net, "name"),
                pins.len().to_string(),
                net["wire_ids"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0)
                    .to_string(),
                refs.join(", "),
            ]
        })
        .collect();
    format!(
        "ネット数: {}\n\n{}",
        nets.len(),
        table(&["ネット", "ピン", "配線", "接続"], &rows)
    )
}

/// Patch配列から (最終revision, op総数) を取る。
fn patches_summary(patches: &Value) -> Option<(String, usize)> {
    let arr = patches.as_array()?;
    let last = arr.last()?;
    let ops = arr
        .iter()
        .map(|p| p["ops"].as_array().map(|a| a.len()).unwrap_or(0))
        .sum();
    Some((text(last, "revision"), ops))
}

/// `madake exec`
pub fn exec_result(command_count: usize, patches: &Value) -> String {
    match patches_summary(patches) {
        Some((revision, ops)) => format!(
            "{command_count} 件のコマンドを実行しました (revision {revision}, 変更 {ops} 件)"
        ),
        None => format!("{command_count} 件のコマンドを実行しました (変更なし)"),
    }
}

/// `madake undo` / `madake redo` (Patch or null)
pub fn history_result(patch: &Value, done: &str, empty: &str) -> String {
    if patch.is_null() {
        return empty.to_string();
    }
    format!(
        "{done} (revision {}, 変更 {} 件)",
        text(patch, "revision"),
        patch["ops"].as_array().map(|a| a.len()).unwrap_or(0)
    )
}

fn written_path(result: &Value) -> String {
    match result["written"].as_str() {
        Some(p) => p.to_string(),
        None => "(不明)".to_string(),
    }
}

/// `madake save`
pub fn saved(result: &Value) -> String {
    format!("保存しました: {}", written_path(result))
}

/// `madake open`
pub fn opened(patch: &Value, path: &str) -> String {
    format!(
        "読み込みました: {} (revision {})",
        path,
        text(patch, "revision")
    )
}

/// `madake export ...`
pub fn exported(kind: ExportKind, result: &Value) -> String {
    format!(
        "{} を書き出しました: {}",
        kind.label(),
        written_path(result)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_snapshot() -> Value {
        json!({
            "revision": 12,
            "can_undo": true,
            "can_redo": false,
            "project": {
                "format_version": 1,
                "name": "サンプル図面",
                "wire_parts": [{ "part_no": "SAMPLE0001", "color": "水色", "sq": 0.3, "note": "" }],
                "sheets": [
                    {
                        "id": "2f4e0b9a-0000-4000-8000-000000000001",
                        "name": "Sheet1",
                        "size": "A3",
                        "orientation": "Landscape",
                        "entities": {
                            "a": { "kind": "symbol", "id": "a", "reference": "K1" },
                            "b": { "kind": "wire", "id": "b" },
                            "c": { "kind": "wire", "id": "c" }
                        }
                    },
                    {
                        "id": "2f4e0b9a-0000-4000-8000-000000000002",
                        "name": "電源系統",
                        "size": "A4",
                        "orientation": "Portrait",
                        "entities": {}
                    }
                ]
            }
        })
    }

    fn sample_netlist() -> Value {
        json!([
            {
                "name": "N001",
                "pins": [
                    { "reference": "K1", "entity_id": "a", "pin": "A1" },
                    { "reference": "F2", "entity_id": "b", "pin": "2" }
                ],
                "wire_ids": ["w1"]
            },
            { "name": "GND", "pins": [], "wire_ids": ["w2", "w3"] }
        ])
    }

    #[test]
    fn disp_width_counts_fullwidth_as_two() {
        assert_eq!(disp_width("abc"), 3);
        assert_eq!(disp_width("シート"), 6);
        assert_eq!(disp_width("K1相"), 4);
    }

    #[test]
    fn pad_uses_display_width() {
        assert_eq!(pad("abc", 5), "abc  ");
        assert_eq!(pad("シート", 8), "シート  ");
        // 既に幅を超えている場合は切り詰めない
        assert_eq!(pad("abcdef", 3), "abcdef");
    }

    #[test]
    fn table_aligns_columns() {
        let out = table(
            &["名前", "数"],
            &[
                vec!["Sheet1".into(), "3".into()],
                vec!["電源系統".into(), "12".into()],
            ],
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        // 最終列の開始表示位置がヘッダ・全データ行で揃う
        let last_col_offset = |line: &str| {
            let idx = line.rfind(' ').map(|i| i + 1).unwrap_or(0);
            disp_width(&line[..idx])
        };
        assert_eq!(last_col_offset(lines[0]), 10); // 「電源系統」(8) + ガター(2)
        assert_eq!(last_col_offset(lines[1]), 10);
        assert_eq!(last_col_offset(lines[2]), 10);
        assert!(lines[1].contains("Sheet1"));
        // 末尾に余分な空白を残さない
        assert!(lines.iter().all(|l| !l.ends_with(' ')));
    }

    #[test]
    fn status_summarizes_connection_and_document() {
        let health = json!({ "name": "MadakeCAD Link API", "version": 1 });
        let out = status(&health, &sample_snapshot(), 9310);
        assert!(out.contains("127.0.0.1:9310"));
        assert!(out.contains("MadakeCAD Link API"));
        assert!(out.contains("サンプル図面"));
        assert!(out.contains("12")); // revision
        assert!(out.contains("Sheet1"));
        assert!(out.contains("3")); // エンティティ総数
        assert!(out.contains("可 / 不可")); // undo可 / redo不可
    }

    #[test]
    fn project_lists_sheets_with_entity_counts() {
        let out = project(&sample_snapshot());
        assert!(out.contains("サンプル図面"));
        assert!(out.contains("2f4e0b9a-0000-4000-8000-000000000001"));
        assert!(out.contains("Sheet1"));
        assert!(out.contains("電源系統"));
        assert!(out.contains("A3"));
        assert!(out.contains("SAMPLE0001"));
        // Sheet1: シンボル1 / 配線2
        let row = out
            .lines()
            .find(|l| l.contains("Sheet1"))
            .expect("Sheet1の行");
        let cols: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(cols[cols.len() - 5], "1"); // symbol
        assert_eq!(cols[cols.len() - 4], "2"); // wire
    }

    #[test]
    fn netlist_renders_table_with_pin_references() {
        let out = netlist(&sample_netlist());
        assert!(out.contains("ネット数: 2"));
        let row = out.lines().find(|l| l.contains("N001")).expect("N001の行");
        assert!(row.contains("K1:A1"));
        assert!(row.contains("F2:2"));
        let gnd = out.lines().find(|l| l.contains("GND")).expect("GNDの行");
        assert!(gnd.contains('2')); // wire_ids 2本
    }

    #[test]
    fn netlist_handles_empty() {
        assert!(netlist(&json!([])).contains("ネットがありません"));
    }

    #[test]
    fn exec_result_reports_revision_and_op_count() {
        let patches = json!([
            { "revision": 14, "ops": [{ "op": "entity_upserted" }] },
            { "revision": 15, "ops": [{ "op": "entity_upserted" }, { "op": "entity_removed" }] }
        ]);
        let out = exec_result(2, &patches);
        assert!(out.contains("2 件"));
        assert!(out.contains("revision 15"));
        assert!(out.contains("3")); // 変更op合計
    }

    #[test]
    fn history_result_handles_null_patch() {
        let out = history_result(
            &Value::Null,
            "元に戻しました",
            "元に戻せる操作がありません。",
        );
        assert_eq!(out, "元に戻せる操作がありません。");
    }

    #[test]
    fn history_result_reports_revision() {
        let patch = json!({ "revision": 11, "ops": [{ "op": "entity_removed" }] });
        let out = history_result(&patch, "元に戻しました", "元に戻せる操作がありません。");
        assert!(out.starts_with("元に戻しました"));
        assert!(out.contains("revision 11"));
    }

    #[test]
    fn saved_and_exported_report_written_path() {
        let res = json!({ "written": "/tmp/a.mdkproj" });
        assert!(saved(&res).contains("/tmp/a.mdkproj"));
        let svg = json!({ "written": "/tmp/a.svg" });
        let out = exported(ExportKind::Svg, &svg);
        assert!(out.contains("SVG"));
        assert!(out.contains("/tmp/a.svg"));
        let wl = json!({ "written": "/tmp/w.csv" });
        assert!(exported(ExportKind::WireList, &wl).contains("電線リスト"));
    }

    #[test]
    fn opened_reports_path_and_revision() {
        let patch = json!({ "revision": 1, "ops": [{ "op": "project_replaced" }] });
        let out = opened(&patch, "/tmp/a.mdkproj");
        assert!(out.contains("/tmp/a.mdkproj"));
        assert!(out.contains("revision 1"));
    }
}
