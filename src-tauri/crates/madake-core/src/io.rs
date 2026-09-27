use std::collections::BTreeMap;
use std::path::Path;

use crate::model::Project;
use crate::Result;

/// 他形式への書き出し結果の内訳 (DXF・KiCad共通)。
///
/// 「何を何件出して、何を出さなかったか」を呼び出し元 (CLI・MCP・UIのコマンドログ) へ
/// 返すための報告。**対応の無い種別を黙って落とさない**ための仕組みで、
/// [`crate::dxf::sheet_to_dxf`] と [`crate::kicad::export_kicad_sch`] が同じ型で返す。
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ExportReport {
    /// 書き出した種別 ([`crate::model::Entity`]のJSON種別タグ) → 件数。0件の種別は入らない。
    pub counts: BTreeMap<String, usize>,
    /// 出さなかった種別と理由 (例 `"shape x3: KiCadの回路図に対応する図形が無い"`)。
    pub skipped: Vec<String>,
}

impl ExportReport {
    /// 書き出した1件を数える。
    pub fn count(&mut self, kind: &str) {
        *self.counts.entry(kind.to_string()).or_default() += 1;
    }

    /// ある種別を出さなかったことを件数と理由付きで記録する (0件なら何も記録しない)。
    pub fn skip(&mut self, kind: &str, count: usize, reason: &str) {
        if count > 0 {
            self.skipped.push(format!("{kind} x{count}: {reason}"));
        }
    }
}

/// プロジェクトを整形JSONで保存する(git差分が読める形式)。
pub fn save_project(path: &Path, project: &Project) -> Result<()> {
    let json = serde_json::to_string_pretty(project)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// プロジェクトを読み込む。古い形式のファイルは現行形式へ移行してから返す ([`migrate`])。
pub fn load_project(path: &Path) -> Result<Project> {
    let json = std::fs::read_to_string(path)?;
    let mut project: Project = serde_json::from_str(&json)?;
    migrate(&mut project);
    Ok(project)
}

/// 旧形式のプロジェクトを現行形式へ移行する。
///
/// 追加されたフィールドはserdeの既定値 (空リスト等) で埋まるので、ここでやることは
/// 版番号を現行へ上げることだけ。**フィールドの意味を変える変更を入れるときは、
/// ここに版ごとの変換を書く**。
pub fn migrate(project: &mut Project) {
    if project.format_version < crate::model::FORMAT_VERSION {
        project.format_version = crate::model::FORMAT_VERSION;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// A project saved to .mdkproj and loaded back is identical, including all entities.
    /// .mdkprojへ保存して読み直したプロジェクトは、全エンティティを含めて同一である。
    #[test]
    fn project_file_roundtrip() {
        let mut engine = Engine::new(Project::new("roundtrip"));
        let sheet_id = engine.project().sheets[0].id;
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Symbol(SymbolInstance {
                    id: Uuid::new_v4(),
                    symbol_id: "relay_coil".into(),
                    at: Point::new(100.0, 50.0),
                    rotation: 90,
                    mirror: false,
                    reference: "K1".into(),
                    value: "[redacted]".into(),
                    attrs: Default::default(),
                }),
            })
            .unwrap();

        let dir = std::env::temp_dir().join("madake_core_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip.mdkproj");
        save_project(&path, engine.project()).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.name, "roundtrip");
        assert_eq!(loaded.sheets.len(), 1);
        assert_eq!(loaded.sheets[0].entities.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    /// The saved .mdkproj file is pretty-printed JSON with a format_version field, so it diffs well in git.
    /// 保存された.mdkprojはformat_version付きの整形JSONで、gitの差分が読みやすい。
    #[test]
    fn saved_file_is_pretty_json_with_format_version() {
        let engine = Engine::new(Project::new("pretty"));
        let dir = std::env::temp_dir().join(format!("madake-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.mdkproj");
        save_project(&path, engine.project()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(&format!("\n  \"format_version\": {FORMAT_VERSION}")),
            "{text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Loading a missing file returns an error instead of panicking.
    /// 存在しないファイルの読み込みはパニックせずエラーを返す。
    #[test]
    fn loading_missing_file_is_an_error() {
        assert!(load_project(std::path::Path::new("/nonexistent/x.mdkproj")).is_err());
    }

    /// A project file saved before the PLC assignment table existed still opens: it gets an empty table and is brought up to the current format version.
    /// PLC割付表が無かった頃の古いプロジェクトファイルもそのまま開ける (割付表は空になり、現行のファイル形式へ更新される)。
    #[test]
    fn an_old_project_file_opens_with_an_empty_plc_assignment_table() {
        let dir = std::env::temp_dir().join(format!("madake-io-old-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 1の頃のファイル (plc_assignmentsが無い)
        std::fs::write(
            &path,
            r#"{"format_version":1,"name":"旧図面","sheets":[],"wire_parts":[]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.name, "旧図面");
        assert!(loaded.plc_assignments.is_empty());
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before frame templates existed opens with no frame template set, which means it is drawn with the default JIS A3 frame exactly as before.
    /// 図枠テンプレートが無かった頃のプロジェクトファイルは図枠未指定で開き、従来どおり既定のJIS A3図枠で描かれる。
    #[test]
    fn an_old_project_file_opens_with_the_default_frame_template() {
        let dir = std::env::temp_dir().join(format!("madake-io-frame-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 2の頃のファイル (frame_templateが無い)
        std::fs::write(
            &path,
            r#"{"format_version":2,"name":"旧図面","sheets":[],"wire_parts":[],"plc_assignments":[]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.frame_template, None);
        assert_eq!(
            crate::frame_template::for_project(&loaded).id,
            crate::frame_template::DEFAULT_FRAME_TEMPLATE_ID
        );
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before panel layouts existed opens with every sheet as a schematic sheet and is brought up to the current format version.
    /// 盤レイアウトが無かった頃のプロジェクトファイルは全シートが回路図シートとして開き、現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_with_schematic_sheets() {
        let dir = std::env::temp_dir().join(format!("madake-io-panel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 3の頃のファイル (シートに種別が無い)
        std::fs::write(
            &path,
            r#"{"format_version":3,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"22222222-2222-4222-8222-222222222222","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.sheets[0].kind, SheetKind::Schematic);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before shapes existed opens exactly as it was, with no shapes on its sheets, and is brought up to the current format version.
    /// 図形が無かった頃のプロジェクトファイルはそのままの姿で開き (シートに図形は無い)、現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_without_any_shapes() {
        let dir = std::env::temp_dir().join(format!("madake-io-shape-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 7の頃のファイル (図形エンティティがまだ無い)
        std::fs::write(
            &path,
            r#"{"format_version":7,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"55555555-5555-4555-8555-555555555555","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.sheets.len(), 1);
        assert_eq!(crate::shape::shapes(&loaded.sheets[0]).count(), 0);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file from the version that introduced shapes still opens with its shapes intact, so a drawing made back then looks the same today.
    /// 図形が導入された版のプロジェクトファイルは、図形を保ったまま今でも開く (当時描いた図面の見た目が変わらない)。
    #[test]
    fn a_project_file_from_the_version_that_introduced_shapes_keeps_them() {
        let dir = std::env::temp_dir().join(format!("madake-io-shape-v8-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v8.mdkproj");
        // 図形エンティティが入った当時 (format_version 8) のファイル
        std::fs::write(
            &path,
            r#"{"format_version":8,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"66666666-6666-4666-8666-666666666666","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb":{"kind":"shape",
                "id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","shape":"rect",
                "points":[{"x":0.0,"y":0.0},{"x":10.0,"y":10.0}],"stroke":"default",
                "stroke_width_mm":0.3,"dashed":false}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(crate::shape::shapes(&loaded.sheets[0]).count(), 1, "図形はv8から入っている");
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before the note leader line existed opens with every note's leader target unset, so no note draws a leader line, and is brought up to the current format version.
    /// 注記の引き出し線が無かった頃のプロジェクトファイルは、全ての注記の引き出し先が未設定として開く (どの注記も引き出し線を描かない)。現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_with_no_leader_targets() {
        let dir = std::env::temp_dir().join(format!("madake-io-leader-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 8の頃のファイル (注記にleader_toが無い)
        std::fs::write(
            &path,
            r#"{"format_version":8,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"99999999-9999-4999-8999-999999999999","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa":{"kind":"text",
                "id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","at":{"x":10.0,"y":20.0},
                "text":"注意","height":2.5,"rotation":0}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        let Some(Entity::Text(t)) = loaded.sheets[0].entities.values().next() else {
            panic!("注記が残っていること")
        };
        assert_eq!(t.leader_to, None);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A shape drawn on a sheet survives a save and reload with its kind, points and look unchanged.
    /// シートに描いた図形は、種類・頂点・見た目を保ったまま保存して読み直せる。
    #[test]
    fn a_shape_survives_a_save_and_reload() {
        let mut project = Project::new("shape");
        let mut shape = crate::shape::new_shape(
            ShapeKind::Rect,
            vec![Point::new(50.0, 50.0), Point::new(150.0, 100.0)],
        );
        shape.stroke = "accent".into();
        shape.stroke_width_mm = 0.5;
        shape.dashed = true;
        shape.fill = Some("muted".into());
        let id = shape.id;
        let entity = Entity::Shape(shape);
        project.sheets[0].entities.insert(entity.id(), entity);

        let dir = std::env::temp_dir().join(format!("madake-io-shape-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shape.mdkproj");
        save_project(&path, &project).unwrap();
        let loaded = load_project(&path).unwrap();
        let Entity::Shape(back) = &loaded.sheets[0].entities[&id] else {
            panic!("図形であること")
        };
        assert_eq!(back.shape, ShapeKind::Rect);
        assert_eq!(back.points.len(), 2);
        assert_eq!(back.stroke, "accent");
        assert_eq!(back.stroke_width_mm, 0.5);
        assert!(back.dashed);
        assert_eq!(back.fill.as_deref(), Some("muted"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A panel sheet and the panel entities on it survive a save and reload unchanged.
    /// 盤レイアウトシートと、その上の盤エンティティは保存して読み直しても変わらない。
    #[test]
    fn a_panel_sheet_survives_a_save_and_reload() {
        let mut project = Project::new("panel");
        let mut panel = Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let footprint = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(80.0, 80.0),
            w: 25.0,
            h: 65.0,
            rotation: 90,
            reference: "K1".into(),
            part_no: "MDK-RLY-24V".into(),
            item_no: None,
        });
        panel.entities.insert(footprint.id(), footprint);
        project.sheets.push(panel);

        let dir = std::env::temp_dir().join(format!("madake-io-panel2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.mdkproj");
        save_project(&path, &project).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.sheets[1].kind, SheetKind::Panel);
        let Some(Entity::Footprint(f)) = loaded.sheets[1].entities.values().next() else {
            panic!("フットプリントが残っていること")
        };
        assert_eq!((f.reference.as_str(), f.rotation), ("K1", 90));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before text styling existed opens with every note unstyled (left-aligned, default colour, not bold) and is brought up to the current format version.
    /// テキスト装飾が無かった頃のプロジェクトファイルは、全ての注記が装飾なし (左揃え・既定色・太字でない) で開き、現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_with_unstyled_notes() {
        let dir = std::env::temp_dir().join(format!("madake-io-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 4の頃のファイル (注記に anchor / color / bold が無い)
        std::fs::write(
            &path,
            r#"{"format_version":4,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"44444444-4444-4444-8444-444444444444","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{"55555555-5555-4555-8555-555555555555":{"kind":"text",
                "id":"55555555-5555-4555-8555-555555555555","at":{"x":10.0,"y":20.0},
                "text":"注意","height":2.5,"rotation":0}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        let Some(Entity::Text(t)) = loaded.sheets[0].entities.values().next() else {
            panic!("注記が残っていること")
        };
        assert_eq!(t.anchor, TextAnchor::Start);
        assert_eq!(t.color, None);
        assert!(!t.bold);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The current file format version is 12, the version that folded the FreeCAD integration (mechanical links and the wire length source) in.
    /// 現行のファイル形式バージョンは12 (FreeCAD連携=機械側の対応付けと電線長の出所を統合した版) である。
    #[test]
    fn the_current_format_version_is_twelve() {
        assert_eq!(FORMAT_VERSION, 12);
        let mut old = Project::new("x");
        old.format_version = 11;
        migrate(&mut old);
        assert_eq!(old.format_version, 12);
    }

    /// A project file saved before the wire part-number legend block existed opens with no legend blocks at all, and is brought up to the current format version.
    /// 電線品番凡例ブロックが無かった頃のプロジェクトファイルは凡例ブロックの無い状態で開き、現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_without_any_wire_legends() {
        let dir = std::env::temp_dir().join(format!("madake-io-wirelegend-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 10の頃のファイル (電線品番凡例ブロックがまだ無い)
        std::fs::write(
            &path,
            r#"{"format_version":10,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.sheets.len(), 1);
        assert_eq!(crate::wire_legend::legends(&loaded.sheets[0]).count(), 0);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A wire part-number legend block placed on a sheet survives a save and reload with its place unchanged.
    /// シートに置いた電線品番凡例ブロックは、置き場所を保ったまま保存して読み直せる。
    #[test]
    fn a_wire_legend_survives_a_save_and_reload() {
        let mut project = Project::new("wire-legend");
        let legend = Entity::WireLegend(WireLegend { id: Uuid::new_v4(), at: Point::new(300.0, 200.0) });
        let id = legend.id();
        project.sheets[0].entities.insert(id, legend);

        let dir = std::env::temp_dir().join(format!("madake-io-wirelegend-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wire-legend.mdkproj");
        save_project(&path, &project).unwrap();
        let loaded = load_project(&path).unwrap();
        let Entity::WireLegend(back) = &loaded.sheets[0].entities[&id] else {
            panic!("電線品番凡例ブロックであること")
        };
        assert_eq!(back.at, Point::new(300.0, 200.0));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A project file saved before the project number existed opens with no project number set, so the `{project.no}` title-block bind stays blank exactly as before, and is brought up to the current format version.
    /// プロジェクト番号が無かった頃のプロジェクトファイルはプロジェクト番号が未設定として開き、表題欄バインド`{project.no}`はこれまでどおり空欄のままになる。現行のファイル形式へ更新される。
    #[test]
    fn an_old_project_file_opens_with_no_project_number() {
        let dir = std::env::temp_dir().join(format!("madake-io-projectno-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v9.mdkproj");
        // format_version 9の頃のファイル (プロジェクトに`no`が無い)
        std::fs::write(
            &path,
            r#"{"format_version":9,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"77777777-7777-4777-8777-777777777777","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.no, None);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A drawing saved before the annotation text height existed opens at the default 2.0mm, so its notes look exactly as before.
    /// 注記文字高が無かった頃の図面は既定の2.0mmで開くので、注記の見た目はこれまでと変わらない。
    #[test]
    fn an_old_project_file_opens_with_the_default_annotation_text_height() {
        let dir = std::env::temp_dir().join(format!("madake-io-annot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 6の頃のファイル (シートに annotation_text_mm が無い)
        std::fs::write(
            &path,
            r#"{"format_version":6,"name":"旧図面","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"88888888-8888-4888-8888-888888888888","name":"S1","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.sheets[0].annotation_text_mm, None, "設定なしとして開く");
        assert!((loaded.sheets[0].annotation_text_mm() - 2.0).abs() < 1e-9);
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The annotation text height set on a sheet is saved to the file and comes back on reload.
    /// シートに設定した注記文字高はファイルへ保存され、読み直しても残っている。
    #[test]
    fn the_annotation_text_height_survives_a_save_and_reload() {
        let mut engine = Engine::new(Project::new("annot"));
        let sheet_id = engine.project().sheets[0].id;
        engine
            .execute(crate::command::Command::SetSheetAnnotationText {
                sheet_id,
                height_mm: Some(1.4),
            })
            .unwrap();
        let dir = std::env::temp_dir().join(format!("madake-io-annot2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.mdkproj");
        save_project(&path, engine.project()).unwrap();
        assert_eq!(load_project(&path).unwrap().sheets[0].annotation_text_mm, Some(1.4));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A panel drawn before item numbers existed opens with no balloons at all: the footprints keep their place and can be numbered whenever the drawing is next tidied up.
    /// アイテム番号が無かった頃の盤図面はバルーンの無い状態で開く。フットプリントの位置はそのままで、番号はいつでも振り直せる。
    #[test]
    fn an_old_panel_file_opens_without_item_numbers() {
        let dir = std::env::temp_dir().join(format!("madake-io-item-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 5の頃のファイル (フットプリントに item_no が無い)
        std::fs::write(
            &path,
            r#"{"format_version":5,"name":"旧盤図","wire_parts":[],"plc_assignments":[],
                "sheets":[{"id":"66666666-6666-4666-8666-666666666666","name":"P1","size":"A3",
                "orientation":"Landscape","kind":"panel","zone_cols":4,"zone_rows":6,"title_block":{},
                "entities":{"77777777-7777-4777-8777-777777777777":{"kind":"footprint",
                "id":"77777777-7777-4777-8777-777777777777","at":{"x":100.0,"y":100.0},
                "w":20.0,"h":40.0,"rotation":0,"reference":"K1","part_no":"MDK-RLY-24V"}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        let Some(Entity::Footprint(fp)) = loaded.sheets[0].entities.values().next() else {
            panic!("フットプリントが残っていること")
        };
        assert_eq!(fp.item_no, None, "採番前として開く");
        assert_eq!(fp.reference, "K1");
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The frame template chosen for a project is saved to the file and comes back on reload.
    /// プロジェクトに選んだ図枠テンプレートはファイルへ保存され、読み直しても残っている。
    #[test]
    fn the_frame_template_survives_a_save_and_reload() {
        let mut engine = Engine::new(Project::new("frame"));
        engine
            .execute(crate::command::Command::SetFrameTemplate {
                frame_template: Some("jis-a4-portrait".into()),
            })
            .unwrap();
        let dir = std::env::temp_dir().join(format!("madake-io-frame2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.mdkproj");
        save_project(&path, engine.project()).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.frame_template.as_deref(), Some("jis-a4-portrait"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A format 2 file (no FreeCAD links, wires without a length source) opens with an empty link list and every wire length marked as manual.
    /// 形式2のファイル(FreeCAD対応付けが無く、配線に長さの出所が無い)は、対応付けが空で全ての配線の長さが手入力扱いとして開ける。
    #[test]
    fn a_format_2_file_opens_with_empty_mech_links_and_manual_lengths() {
        let dir = std::env::temp_dir().join(format!("madake-io-v2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v2.mdkproj");
        std::fs::write(
            &path,
            r#"{"format_version":2,"name":"v2","wire_parts":[],"plc_assignments":[],"sheets":[{"id":"00000000-0000-0000-0000-000000000001","name":"S","size":"A3","orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},"revisions":[],"entities":{"00000000-0000-0000-0000-000000000002":{"kind":"wire","id":"00000000-0000-0000-0000-000000000002","points":[{"x":0,"y":0},{"x":10,"y":0}],"color":"red","sq":0.75,"length_m":0.5}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert!(loaded.mech_links.is_empty());
        assert_eq!(loaded.format_version, FORMAT_VERSION);
        let Entity::Wire(w) = loaded.sheets[0].entities.values().next().unwrap() else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(0.5), LengthSource::Manual));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A format 3 file whose FreeCAD links have no placement opens with those links unplaced, and a link with a placement round-trips through save and load.
    /// 形式3のファイル(FreeCAD対応付けに配置が無い)は配置なしの対応付けとして開け、配置付きの対応付けは保存→読み込みで保たれる。
    #[test]
    fn a_format_3_file_opens_with_unplaced_links_and_placements_round_trip() {
        let dir = std::env::temp_dir().join(format!("madake-io-v3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v3.mdkproj");
        std::fs::write(
            &path,
            r#"{"format_version":3,"name":"v3","sheets":[],"mech_links":[{"entity_id":"00000000-0000-0000-0000-000000000002","fcstd_path":"/p.FCStd","object_name":"Relay001","synced_at":""}]}"#,
        )
        .unwrap();
        let mut loaded = load_project(&path).unwrap();
        assert_eq!(loaded.format_version, FORMAT_VERSION);
        assert_eq!(loaded.mech_links[0].placement, None);
        loaded.mech_links[0].placement = Some(MechPlacement { x_mm: 120.0, y_mm: 45.5, z_mm: 0.0, rotation_deg: 90.0 });
        save_project(&path, &loaded).unwrap();
        let again = load_project(&path).unwrap();
        assert_eq!(again.mech_links[0].placement, Some(MechPlacement { x_mm: 120.0, y_mm: 45.5, z_mm: 0.0, rotation_deg: 90.0 }));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A file written by the cloud build (its own format 4, with FreeCAD links and a wire length measured in FreeCAD) opens with those fields intact and is rewritten as the current merged format version.
    /// クラウド側のビルドが書いたファイル(独自の形式4。FreeCAD対応付けとFreeCAD計測の電線長を含む)は、それらの内容を保ったまま開き、統合後の現行形式で書き直される。
    #[test]
    fn a_cloud_format_4_file_keeps_its_mech_links_and_length_source() {
        let dir = std::env::temp_dir().join(format!("madake-io-cloud-v4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cloud-v4.mdkproj");
        // クラウド側が「format_version 4」として書いたファイル (ローカル側のv4とは別系統)
        std::fs::write(
            &path,
            r#"{"format_version":4,"name":"cloud","wire_parts":[],"plc_assignments":[],
                "mech_links":[{"entity_id":"00000000-0000-0000-0000-000000000002",
                "fcstd_path":"/p.FCStd","object_name":"Relay001","synced_at":"2026-09-27T00:00:00Z",
                "placement":{"x_mm":120.0,"y_mm":45.5,"z_mm":0.0,"rotation_deg":90.0}}],
                "sheets":[{"id":"00000000-0000-0000-0000-000000000001","name":"S","size":"A3",
                "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},"revisions":[],
                "entities":{"00000000-0000-0000-0000-000000000002":{"kind":"wire",
                "id":"00000000-0000-0000-0000-000000000002","points":[{"x":0.0,"y":0.0},{"x":10.0,"y":0.0}],
                "color":"red","sq":0.75,"length_m":1.25,"length_source":"freecad"}}}]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.mech_links.len(), 1, "FreeCAD対応付けが残っていること");
        assert_eq!(loaded.mech_links[0].object_name, "Relay001");
        assert_eq!(
            loaded.mech_links[0].placement,
            Some(MechPlacement { x_mm: 120.0, y_mm: 45.5, z_mm: 0.0, rotation_deg: 90.0 })
        );
        let Some(Entity::Wire(w)) = loaded.sheets[0].entities.values().next() else {
            panic!("配線が残っていること")
        };
        assert_eq!((w.length_m, w.length_source), (Some(1.25), LengthSource::Freecad));
        assert_eq!(loaded.format_version, FORMAT_VERSION, "統合後の現行形式(12)へ更新される");

        // 書き直したファイルも現行形式で、読み直しても中身は変わらない
        save_project(&path, &loaded).unwrap();
        let again = load_project(&path).unwrap();
        assert_eq!(again.format_version, 12);
        assert_eq!(again.mech_links, loaded.mech_links);
        std::fs::remove_dir_all(&dir).ok();
    }
}
