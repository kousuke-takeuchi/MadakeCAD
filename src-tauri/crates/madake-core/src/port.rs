//! ポート (接点を持つピン、[`crate::symbol::PinDef::contacts`]) の ERC。
//!
//! 材料はネットリストが作る ([`crate::netlist::port_report`])。ここは文と重さを決めるだけ。

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Project, Sheet, SheetId};
use crate::symbol::sheet_symbol_defs;
use crate::verify::{Diagnostic, Severity};

/// ポートのネットにいる、接点の分からない普通のピン (警告)。
pub const PORT_TO_PLAIN_PIN: &str = "erc.port_to_plain_pin";
/// 繋がったポートのうち1つにしか無い接点 (情報)。
pub const PORT_CONTACT_UNMATCHED: &str = "erc.port_contact_unmatched";
/// あるシートではポートのラベルなのに、別のシートではポートの無いネットに載っているラベル (警告)。
pub const PORT_LABEL_UNSPLIT: &str = "erc.port_label_unsplit";

/// シート1枚のポートの ERC。
pub fn check_ports(sheet: &Sheet) -> Vec<Diagnostic> {
    let defs = sheet_symbol_defs(sheet);
    let report = crate::netlist::port_report(sheet, &defs);
    let diag = |severity: Severity, code: &str, message: String, ids: Vec<crate::model::EntityId>| Diagnostic {
        severity,
        code: code.into(),
        message,
        sheet_id: sheet.id,
        entity_ids: ids,
    };
    let mut out = Vec::new();
    for p in &report.plain_pins {
        out.push(diag(
            Severity::Warning,
            PORT_TO_PLAIN_PIN,
            format!(
                "{} は {} ({}) につながっていますが、どの接点か分かりません。ピン名を接点名にするか、connect_cable で芯ごとに引く",
                p.pin,
                p.ports.join(", "),
                p.contacts.join(", ")
            ),
            vec![p.entity_id],
        ));
    }
    for l in &report.lonely_contacts {
        out.push(diag(
            Severity::Info,
            PORT_CONTACT_UNMATCHED,
            format!("接点 {} は {} にしかありません (同じネットのほかのポートには無い)", l.contact, l.port),
            l.entity_ids.clone(),
        ));
    }
    for s in &report.stray_cores {
        out.push(diag(
            Severity::Error,
            crate::cable::CABLE_CORE_UNASSIGNED,
            format!("ケーブル {} の芯 {}: {}", s.reference, s.core, s.why),
            vec![s.cable_id],
        ));
    }
    out
}

/// プロジェクトのポートの ERC: ポートのラベルが、別のシートでポートの無いネットに載っていないか。
pub fn port_label_diagnostics(project: &Project) -> Vec<Diagnostic> {
    let mut port_sheets: BTreeMap<String, BTreeSet<SheetId>> = BTreeMap::new();
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        for name in crate::netlist::port_report(sheet, &defs).port_labels {
            port_sheets.entry(name).or_default().insert(sheet.id);
        }
    }
    let mut out = Vec::new();
    if port_sheets.is_empty() {
        return out;
    }
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        for net in crate::netlist::extract_netlist(sheet, &defs) {
            if net.contact.is_some() || !net.contact_labels.is_empty() {
                continue;
            }
            let (label_ids, names) = crate::xref::labels_on_net(sheet, &net);
            for name in names {
                let Some(on) = port_sheets.get(&name) else { continue };
                if on.iter().all(|s| *s == sheet.id) {
                    continue;
                }
                let ids: Vec<_> = label_ids
                    .iter()
                    .copied()
                    .filter(|id| matches!(sheet.entities.get(id), Some(crate::model::Entity::NetLabel(l)) if l.name.trim() == name))
                    .collect();
                out.push(Diagnostic {
                    severity: Severity::Warning,
                    code: PORT_LABEL_UNSPLIT.into(),
                    message: format!(
                        "ラベル {name} は別のシートではポートのネットですが、このシートのネットにはポートがありません。シートは繋がりません。ここにもポートを置くか、ラベル {name}.<接点> を使う"
                    ),
                    sheet_id: sheet.id,
                    entity_ids: ids,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cable::test_support::{insert, wire};
    use crate::block::port_fixtures::{pin_at, port_block, USB};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// 2つの部品のピンを配線でつなぐ。
    fn join(sheet: &mut Sheet, a: (EntityId, &str), b: (EntityId, &str)) {
        let (pa, pb) = (pin_at(sheet, a.0, a.1), pin_at(sheet, b.0, b.1));
        insert(sheet, pts(pa, pb));
    }

    fn pts(a: Point, b: Point) -> Entity {
        Entity::Wire(wire(&[(a.x, a.y), (b.x, b.y)]))
    }

    fn label(name: &str, at: Point) -> Entity {
        Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at, name: name.into(), rotation: 0, attrs: Default::default() })
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    /// A plain pin wired to a port whose name matches no contact is a warning that names the pin, the port and its contacts; a pin named like a contact is not.
    /// ポートに繋がった普通のピンで、名前がどの接点とも合わないものは、ピン・ポート・接点を書いた警告になる。接点と同じ名前のピンは警告にならない。
    #[test]
    fn a_plain_pin_on_a_port_is_a_warning() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = insert(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let jst = insert(&mut sheet, crate::block::test_block("J45", "", 200.0, 100.0, r#"{"left":[{"no":"1"},{"no":"4","name":"GND"}]}"#));
        let h = pin_at(&sheet, hub, "U3");
        let p1 = pin_at(&sheet, jst, "1");
        insert(&mut sheet, pts(h, p1));
        let p4 = pin_at(&sheet, jst, "4");
        insert(&mut sheet, pts(h, p4));
        let diags = check_ports(&sheet);
        assert_eq!(codes(&diags), vec![PORT_TO_PLAIN_PIN]);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("J45:1") && diags[0].message.contains("39:U3") && diags[0].message.contains("VBUS, D-, D+, GND"), "{}", diags[0].message);
    }

    /// A contact on only one of two joined ports is reported as info; a port wired only to a mate half gives no such info.
    /// 繋がった2つのポートのうち1つにしか無い接点は情報として知らせる。かみ合わせの片側にだけ繋いだポートでは出さない。
    #[test]
    fn a_contact_on_only_one_of_two_ports_is_reported_as_info() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = insert(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let otg = insert(&mut sheet, Entity::Symbol(port_block("OTG1", 200.0, 100.0, "left", "P1", &["VBUS", "D-", "D+", "GND", "ID"])));
        join(&mut sheet, (hub, "U3"), (otg, "P1"));
        let diags = check_ports(&sheet);
        assert_eq!(codes(&diags), vec![PORT_CONTACT_UNMATCHED]);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("ID") && diags[0].message.contains("OTG1:P1"), "{}", diags[0].message);

        let mut alone = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = insert(&mut alone, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let socket = insert(&mut alone, Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(), symbol_id: "connector_socket".into(), at: Point::new(150.0, 100.0),
            rotation: 0, mirror: false, reference: "J1".into(), value: String::new(), attrs: Default::default(),
        }));
        join(&mut alone, (hub, "U3"), (socket, "1"));
        assert!(check_ports(&alone).is_empty(), "{:?}", check_ports(&alone));
    }

    /// A run-cable core whose contact is no longer on the port pin (the block was edited after) is reported as an unassigned core naming the contact.
    /// 区間ケーブルの芯の接点がポートのピンからなくなった (後でブロックを直した) ときは、その接点を書いた「芯の行き先なし」として知らせる。
    #[test]
    fn a_core_on_a_removed_contact_is_reported() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let mut hub = port_block("39", 200.0, 100.0, "left", "U3", &["VBUS", "GND"]);
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let diags = check_ports(&lte.sheet);
        let stray: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == crate::cable::CABLE_CORE_UNASSIGNED).collect();
        assert_eq!(stray.len(), 2, "{diags:?}");
        assert!(stray.iter().any(|d| d.message.contains("D-") && d.message.contains("39:U3")), "{stray:?}");
        assert_eq!(stray[0].severity, Severity::Error);
    }

    /// A core that names a contact on a pin edited to have no contacts at all is reported as unassigned with that contact named, not silently put on the plain pin.
    /// 接点をすべて消したピンで、接点を指定している芯は、その接点を名指しで割当て無しと報告される (黙って普通のピンに載せない)。
    #[test]
    fn a_core_on_a_pin_without_contacts_is_reported() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let Entity::Symbol(mut hub) = crate::block::test_block("39", "", 200.0, 100.0, r#"{"left":[{"no":"U3"}]}"#) else { panic!() };
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let diags = check_ports(&lte.sheet);
        let stray: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == crate::cable::CABLE_CORE_UNASSIGNED).collect();
        assert_eq!(stray.len(), 4, "{diags:?}");
        assert!(stray.iter().any(|d| d.message.contains("接点 D-") && d.message.contains("39:U3") && d.message.contains("接点: なし")), "{stray:?}");
    }

    /// A label that is a port label on one sheet but sits on a net without a port on another sheet is a project warning, because those sheets are not joined.
    /// あるシートではポートのラベルなのに、別のシートではポートの無いネットに載っているラベルは、シートが繋がらないのでプロジェクトの警告になる。
    #[test]
    fn a_port_label_without_a_port_on_another_sheet_is_a_warning() {
        let mut project = Project::new("t");
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let (s1, s2) = (project.sheets[0].id, project.sheets[1].id);
        let sheet1 = project.sheet_mut(s1).unwrap();
        let hub = insert(sheet1, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let h = pin_at(sheet1, hub, "U3");
        insert(sheet1, pts(h, Point::new(h.x + 20.0, h.y)));
        insert(sheet1, label("USB_MIC", Point::new(h.x + 20.0, h.y)));
        let sheet2 = project.sheet_mut(s2).unwrap();
        insert(sheet2, pts(Point::new(50.0, 50.0), Point::new(80.0, 50.0)));
        let l = insert(sheet2, label("USB_MIC", Point::new(80.0, 50.0)));
        let diags = port_label_diagnostics(&project);
        assert_eq!(codes(&diags), vec![PORT_LABEL_UNSPLIT]);
        assert_eq!((diags[0].sheet_id, diags[0].entity_ids.clone()), (s2, vec![l]));
        assert!(crate::verify::verify_project(&project).iter().any(|d| d.code == PORT_LABEL_UNSPLIT));
    }
}
