//! ブロックのピンのコネクタの絵 (USB・3.5mm・DC・端子台・JST)。
//!
//! ピン表のピンが `connector` (どの形のコネクタか) を持つと、外形の辺にそのコネクタの小さな絵を描く。
//! 絵は既存のプリミティブ (線・円・弧・長方形) だけで作り、ピンが外形と交わる点 E を原点にした
//! ローカル座標 (u = 外向き、v = 辺に沿う向き) で決める。`connector` は絵だけで、接点・ネットは変えない。
//! TS側 `src/canvas/connector.ts` と同じ数・同じ順 (`tests/golden/connector-blocks.json` で突き合わせる)。

use serde::Deserialize;

use crate::block::BlockPin;

/// コネクタの形 (13種、決め打ち)。`_f` = メス (機器側の口)、`_m` = オス (プラグ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    UsbAF,
    UsbAM,
    UsbCF,
    UsbCM,
    MicroBF,
    MicroBM,
    Jack35F,
    Jack35M,
    DcF,
    DcM,
    Screw,
    JstPhF,
    JstXhF,
}

impl ConnectorKind {
    /// 全種類 (仕様の表の順。エディタの選択肢もこの順)。
    pub const ALL: [ConnectorKind; 13] = [
        ConnectorKind::UsbAF,
        ConnectorKind::UsbAM,
        ConnectorKind::UsbCF,
        ConnectorKind::UsbCM,
        ConnectorKind::MicroBF,
        ConnectorKind::MicroBM,
        ConnectorKind::Jack35F,
        ConnectorKind::Jack35M,
        ConnectorKind::DcF,
        ConnectorKind::DcM,
        ConnectorKind::Screw,
        ConnectorKind::JstPhF,
        ConnectorKind::JstXhF,
    ];

    /// JSON での名前 (`usb_a_f` など)。
    pub fn name(self) -> &'static str {
        match self {
            ConnectorKind::UsbAF => "usb_a_f",
            ConnectorKind::UsbAM => "usb_a_m",
            ConnectorKind::UsbCF => "usb_c_f",
            ConnectorKind::UsbCM => "usb_c_m",
            ConnectorKind::MicroBF => "micro_b_f",
            ConnectorKind::MicroBM => "micro_b_m",
            ConnectorKind::Jack35F => "jack35_f",
            ConnectorKind::Jack35M => "jack35_m",
            ConnectorKind::DcF => "dc_f",
            ConnectorKind::DcM => "dc_m",
            ConnectorKind::Screw => "screw",
            ConnectorKind::JstPhF => "jst_ph_f",
            ConnectorKind::JstXhF => "jst_xh_f",
        }
    }

    /// 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。
    pub fn is_grouped(self) -> bool {
        matches!(self, ConnectorKind::Screw | ConnectorKind::JstPhF | ConnectorKind::JstXhF)
    }

    /// JST のシリーズ名 (`PH` / `XH`)。他は `None`。
    pub fn series(self) -> Option<&'static str> {
        match self {
            ConnectorKind::JstPhF => Some("PH"),
            ConnectorKind::JstXhF => Some("XH"),
            _ => None,
        }
    }
}

/// 1辺の中の、1つの絵で描くピンのまとまり (`start..end` は辺のピン表の添字)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorGroup {
    pub kind: ConnectorKind,
    /// 前後の空白を落とした `connector_ref`。無ければ `None`。
    pub reference: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// 1辺のピンをまとまりに分ける。まとめる種類のピンは、すぐ前のピンと種類も `connector_ref` も
/// 同じなら同じまとまりに入る (無い同士も同じ)。まとめない種類は1本で1つ。`connector` の無いピンは入らない。
pub fn connector_groups(side: &[BlockPin]) -> Vec<ConnectorGroup> {
    let mut groups: Vec<ConnectorGroup> = Vec::new();
    for (i, pin) in side.iter().enumerate() {
        let Some(kind) = pin.connector else { continue };
        let reference = pin.connector_ref.as_deref().map(|r| r.trim().to_string());
        if kind.is_grouped() {
            if let Some(last) = groups.last_mut() {
                if last.end == i && last.kind == kind && last.reference == reference {
                    last.end = i + 1;
                    continue;
                }
            }
        }
        groups.push(ConnectorGroup { kind, reference, start: i, end: i + 1 });
    }
    groups
}

/// まとまりの名前: JST は `<ref> PH` / `<ref> XH` (ref が無ければ `PH` / `XH`)、端子台は `<ref>` (無ければ書かない)。
pub fn group_label(group: &ConnectorGroup) -> Option<String> {
    match (group.kind.series(), &group.reference) {
        (Some(series), Some(r)) => Some(format!("{r} {series}")),
        (Some(series), None) => Some(series.to_string()),
        (None, Some(r)) if group.kind.is_grouped() => Some(r.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockPins;

    fn left_side(json: &str) -> Vec<BlockPin> {
        let pins: BlockPins = serde_json::from_str(json).expect("ピン表");
        pins.left
    }

    fn spans(json: &str) -> Vec<(usize, usize)> {
        connector_groups(&left_side(json)).iter().map(|g| (g.start, g.end)).collect()
    }

    fn label(json: &str) -> Option<String> {
        group_label(&connector_groups(&left_side(json))[0])
    }

    /// Every connector kind is read from its snake_case name (usb_a_f … jst_xh_f); only screw terminals and JST sockets are grouped, and only JST has a series.
    /// コネクタの種類はどれも snake_case の名前 (usb_a_f … jst_xh_f) で読め、まとめて描くのは端子台と JST だけで、シリーズ名を持つのは JST だけ。
    #[test]
    fn every_connector_kind_has_its_json_name() {
        let names: Vec<&str> = ConnectorKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "usb_a_f", "usb_a_m", "usb_c_f", "usb_c_m", "micro_b_f", "micro_b_m", "jack35_f",
                "jack35_m", "dc_f", "dc_m", "screw", "jst_ph_f", "jst_xh_f"
            ]
        );
        for kind in ConnectorKind::ALL {
            let read: ConnectorKind = serde_json::from_str(&format!("\"{}\"", kind.name())).expect("読める");
            assert_eq!(read, kind);
        }
        let grouped: Vec<&str> = ConnectorKind::ALL.iter().filter(|k| k.is_grouped()).map(|k| k.name()).collect();
        assert_eq!(grouped, ["screw", "jst_ph_f", "jst_xh_f"]);
        assert_eq!(ConnectorKind::JstPhF.series(), Some("PH"));
        assert_eq!(ConnectorKind::JstXhF.series(), Some("XH"));
        assert_eq!(ConnectorKind::Screw.series(), None);
    }

    /// Consecutive screw or JST pins with the same kind and the same name (or both unnamed) form one connector; a different name, a different kind or a pin in between starts a new one, and USB-like kinds are always one pin each.
    /// 同じ種類で同じ名前 (どちらも名前なしを含む) の端子台・JST のピンが続けば1つのコネクタになり、名前・種類が違うか間に別のピンがあれば別のコネクタになる。USB などはいつも1本で1つ。
    #[test]
    fn consecutive_pins_with_the_same_kind_and_name_form_one_connector() {
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":" TB1 "}]}"#),
            [(0, 2)]
        );
        assert_eq!(spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2","connector":"screw"}]}"#), [(0, 2)]);
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":"TB2"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"jst_ph_f"},{"no":"2","connector":"jst_xh_f"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}]}"#),
            [(0, 1), (2, 3)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"usb_a_f"},{"no":"2","connector":"usb_a_f"}]}"#),
            [(0, 1), (1, 2)]
        );
    }

    /// A JST connector is labelled with its name and series ("J3 PH", or "XH" alone), a screw terminal block with its name, and an unnamed screw block or a USB socket gets no label.
    /// JST のコネクタには名前とシリーズ (「J3 PH」、名前が無ければ「XH」だけ)、端子台には名前を書き、名前の無い端子台と USB の口には何も書かない。
    #[test]
    fn group_labels_name_the_connector_and_its_series() {
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}]}"#).as_deref(), Some("J3 PH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_xh_f"}]}"#).as_deref(), Some("XH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"}]}"#).as_deref(), Some("TB1"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw"}]}"#), None);
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"usb_a_f"}]}"#), None);
    }
}
