//! 部品DB (SQLite、グローバル共有マスタ)。ドキュメント外なのでCommandエンジンは通らない。
//!
//! パス解決: 環境変数 `MADAKE_PARTS_DB` → OSのアプリデータdir/MadakeCAD/parts.sqlite。
//! 図面(.mdkproj)には型番・定格が書き込まれ自己完結を維持する(DBはマスタ、図面はスナップショット)。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// スキーマバージョン (metaテーブルに保存。変更時はマイグレーションを書く)。
/// v2: partsに`spice_model`列を追加 (過渡解析・非線形モデル用のSPICE素子行)。
/// v3: partsに`contact_config`列を追加 (リレーの接点構成。コイル⇔接点XRefの接点数検証用)。
/// v4: partsに`plc_module`列を追加 (PLC I/Oモジュールの定義JSON。点数・入出力・アドレス体系)。
pub const SCHEMA_VERSION: u32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum PartsError {
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 部品マスタの1行。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Part {
    /// 型番 (一意キー)。
    pub part_no: String,
    #[serde(default)]
    pub maker: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    /// 既定シンボルid (動的ID可。例: "fuse", "terminal_block_8p")。
    #[serde(default)]
    pub symbol_id: String,
    /// 定格電圧 (表記のまま。例: "DC24V")。
    #[serde(default)]
    pub rated_voltage: String,
    /// 定格電流 (A)。検証エンジンのattrs.current_aと連動。
    #[serde(default)]
    pub rated_current_a: Option<f64>,
    #[serde(default)]
    pub purchase_url: String,
    #[serde(default)]
    pub datasheet_url: String,
    /// 参考価格。
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub note: String,
    /// 3Dモデル参照 (STEP/FCStdパス。フェーズM用の予約)。
    #[serde(default)]
    pub model_3d: String,
    /// 取付情報 (DINレール/ねじ等。予約)。
    #[serde(default)]
    pub mounting: String,
    /// SPICEモデル (素子行テンプレート。過渡解析・非線形モデル用、v2)。
    #[serde(default)]
    pub spice_model: String,
    /// 接点構成 (リレー・コンタクタの実装数。例 "2NO+2NC"、v3)。
    /// 図面に置くとシンボルの`attrs["contact_config"]`へ写り、接点数超過の検証に使われる。
    #[serde(default)]
    pub contact_config: String,
    /// PLC I/Oモジュールの定義 (JSON。点数・入出力の種別・アドレス体系、v4)。
    /// 空欄ならPLCモジュールではない。読み方は [`crate::plc::PlcModuleSpec::parse`]。
    #[serde(default)]
    pub plc_module: String,
}

fn default_currency() -> String {
    "JPY".into()
}

/// サンプルのPLC I/Oモジュール1件を組み立てる (定義JSONと既定シンボルを揃える)。
fn plc_module_part(
    part_no: &str,
    name: &str,
    points: usize,
    kind: crate::plc::PlcIoKind,
    address_prefix: &str,
    address_style: crate::plc::PlcAddressStyle,
) -> Part {
    let spec = crate::plc::PlcModuleSpec {
        points,
        kind,
        address_prefix: address_prefix.into(),
        address_style,
    };
    Part {
        part_no: part_no.into(),
        name: name.into(),
        category: "plc".into(),
        symbol_id: spec.symbol_id(),
        rated_voltage: "DC24V".into(),
        plc_module: spec.to_json(),
        ..Default::default()
    }
}

/// 電線品番マスタの1行 (線色+sq→品番)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WirePartRow {
    pub part_no: String,
    pub color: String,
    pub sq: f64,
    #[serde(default)]
    pub purchase_url: String,
    #[serde(default)]
    pub price_per_m: Option<f64>,
    #[serde(default)]
    pub note: String,
}

/// 部品DB接続。
pub struct PartsDb {
    conn: rusqlite::Connection,
}

/// 既定のDBパス (env MADAKE_PARTS_DB優先)。
pub fn default_db_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MADAKE_PARTS_DB") {
        return PathBuf::from(p);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("MadakeCAD")
        .join("parts.sqlite")
}

const PART_COLUMNS: &str = "part_no, maker, name, category, symbol_id, rated_voltage, \
     rated_current_a, purchase_url, datasheet_url, price, currency, note, model_3d, mounting, \
     spice_model, contact_config, plc_module";

fn row_to_part(row: &rusqlite::Row<'_>) -> rusqlite::Result<Part> {
    Ok(Part {
        part_no: row.get(0)?,
        maker: row.get(1)?,
        name: row.get(2)?,
        category: row.get(3)?,
        symbol_id: row.get(4)?,
        rated_voltage: row.get(5)?,
        rated_current_a: row.get(6)?,
        purchase_url: row.get(7)?,
        datasheet_url: row.get(8)?,
        price: row.get(9)?,
        currency: row.get(10)?,
        note: row.get(11)?,
        model_3d: row.get(12)?,
        mounting: row.get(13)?,
        spice_model: row.get(14)?,
        contact_config: row.get(15)?,
        plc_module: row.get(16)?,
    })
}

impl PartsDb {
    /// 開く (無ければスキーマ作成+サンプル投入)。
    pub fn open(path: &Path) -> Result<Self, PartsError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS parts (
               part_no TEXT PRIMARY KEY,
               maker TEXT NOT NULL DEFAULT '',
               name TEXT NOT NULL DEFAULT '',
               category TEXT NOT NULL DEFAULT '',
               symbol_id TEXT NOT NULL DEFAULT '',
               rated_voltage TEXT NOT NULL DEFAULT '',
               rated_current_a REAL,
               purchase_url TEXT NOT NULL DEFAULT '',
               datasheet_url TEXT NOT NULL DEFAULT '',
               price REAL,
               currency TEXT NOT NULL DEFAULT 'JPY',
               note TEXT NOT NULL DEFAULT '',
               model_3d TEXT NOT NULL DEFAULT '',
               mounting TEXT NOT NULL DEFAULT '',
               spice_model TEXT NOT NULL DEFAULT '',
               contact_config TEXT NOT NULL DEFAULT '',
               plc_module TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS wire_parts (
               part_no TEXT PRIMARY KEY,
               color TEXT NOT NULL,
               sq REAL NOT NULL,
               purchase_url TEXT NOT NULL DEFAULT '',
               price_per_m REAL,
               note TEXT NOT NULL DEFAULT ''
             );",
        )?;
        let db = Self { conn };
        // schema_versionが無い = 新規作成。バージョンを記録しサンプルを投入する
        let version: Option<u32> = db
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());
        match version {
            None => {
                db.conn.execute(
                    "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
                    [SCHEMA_VERSION.to_string()],
                )?;
                db.seed_samples()?;
            }
            Some(v) if v < SCHEMA_VERSION => db.migrate(v)?,
            _ => {}
        }
        Ok(db)
    }

    /// 旧スキーマからのマイグレーション。
    fn migrate(&self, from: u32) -> Result<(), PartsError> {
        if from < 2 {
            // v1→v2: spice_model列を追加
            self.conn.execute(
                "ALTER TABLE parts ADD COLUMN spice_model TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if from < 3 {
            // v2→v3: contact_config列を追加
            self.conn.execute(
                "ALTER TABLE parts ADD COLUMN contact_config TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        if from < 4 {
            // v3→v4: plc_module列を追加
            self.conn.execute(
                "ALTER TABLE parts ADD COLUMN plc_module TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        self.conn.execute(
            "UPDATE meta SET value=?1 WHERE key='schema_version'",
            [SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
    }

    /// サンプル部品 (ダミー型番)。新規作成時のみ呼ばれる。
    fn seed_samples(&self) -> Result<(), PartsError> {
        let samples = [
            Part {
                part_no: "MDK-FUSE-5A".into(),
                name: "ガラス管ヒューズ 5A".into(),
                category: "protection".into(),
                symbol_id: "fuse".into(),
                rated_voltage: "AC250V".into(),
                rated_current_a: Some(5.0),
                price: Some(50.0),
                ..Default::default()
            },
            Part {
                part_no: "MDK-RLY-24V".into(),
                name: "パワーリレー DC24Vコイル".into(),
                category: "relay".into(),
                symbol_id: "relay_coil".into(),
                rated_voltage: "DC24V".into(),
                rated_current_a: Some(0.05),
                price: Some(900.0),
                // 2c接点 (2回路の切替接点) = a接点2 + b接点2
                contact_config: "2NO+2NC".into(),
                ..Default::default()
            },
            Part {
                part_no: "MDK-RLY-MY2N-DC24".into(),
                name: "小型パワーリレー MY2N相当 2c DC24V".into(),
                category: "relay".into(),
                symbol_id: "relay_coil".into(),
                rated_voltage: "DC24V".into(),
                rated_current_a: Some(0.04),
                price: Some(880.0),
                contact_config: "2NO+2NC".into(),
                ..Default::default()
            },
            Part {
                part_no: "MDK-TB-8P".into(),
                name: "端子台 8極".into(),
                category: "connector".into(),
                symbol_id: "terminal_block_8p".into(),
                rated_voltage: "AC600V".into(),
                rated_current_a: Some(20.0),
                price: Some(450.0),
                ..Default::default()
            },
            Part {
                part_no: "MDK-LAMP-24V".into(),
                name: "表示灯 DC24V".into(),
                category: "output".into(),
                symbol_id: "lamp".into(),
                rated_voltage: "DC24V".into(),
                rated_current_a: Some(0.02),
                price: Some(600.0),
                ..Default::default()
            },
            // PLC I/Oモジュール (M4仕様 §3)。メーカ別のアドレス体系をひと通り揃える
            plc_module_part(
                "MDK-PLC-DI16-MITSUBISHI",
                "PLC入力ユニット 16点 (三菱 FX5-16EX相当)",
                16,
                crate::plc::PlcIoKind::Di,
                "X",
                crate::plc::PlcAddressStyle::Mitsubishi,
            ),
            plc_module_part(
                "MDK-PLC-DO16-MITSUBISHI",
                "PLC出力ユニット 16点 トランジスタ (三菱 FX5-16EYT相当)",
                16,
                crate::plc::PlcIoKind::Do,
                "Y",
                crate::plc::PlcAddressStyle::Mitsubishi,
            ),
            plc_module_part(
                "MDK-PLC-DI8-SIEMENS",
                "PLC入力ユニット 8点 (Siemens SM1221相当)",
                8,
                crate::plc::PlcIoKind::Di,
                "%I",
                crate::plc::PlcAddressStyle::Siemens,
            ),
            Part {
                part_no: "MDK-CONN-3P".into(),
                name: "コネクタ 3極".into(),
                category: "connector".into(),
                symbol_id: "connector_3p".into(),
                rated_current_a: Some(3.0),
                price: Some(120.0),
                ..Default::default()
            },
        ];
        for p in &samples {
            let mut sample = p.clone();
            sample.maker = "サンプル".into();
            sample.note = "同梱サンプル (ダミー型番)".into();
            sample.currency = default_currency();
            self.upsert_part(&sample)?;
        }
        for (part_no, color, sq) in [
            ("MDK-W-03SB", "light_blue", 0.3),
            ("MDK-W-075RD", "red", 0.75),
            ("MDK-W-075BK", "black", 0.75),
            ("MDK-W-20WH", "white", 2.0),
        ] {
            self.upsert_wire_part(&WirePartRow {
                part_no: part_no.into(),
                color: color.into(),
                sq,
                purchase_url: String::new(),
                price_per_m: None,
                note: "同梱サンプル".into(),
            })?;
        }
        Ok(())
    }

    pub fn upsert_part(&self, part: &Part) -> Result<(), PartsError> {
        self.conn.execute(
            &format!(
                "INSERT INTO parts ({PART_COLUMNS}) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17) \
                 ON CONFLICT(part_no) DO UPDATE SET \
                 maker=?2, name=?3, category=?4, symbol_id=?5, rated_voltage=?6, \
                 rated_current_a=?7, purchase_url=?8, datasheet_url=?9, price=?10, \
                 currency=?11, note=?12, model_3d=?13, mounting=?14, spice_model=?15, \
                 contact_config=?16, plc_module=?17"
            ),
            rusqlite::params![
                part.part_no,
                part.maker,
                part.name,
                part.category,
                part.symbol_id,
                part.rated_voltage,
                part.rated_current_a,
                part.purchase_url,
                part.datasheet_url,
                part.price,
                part.currency,
                part.note,
                part.model_3d,
                part.mounting,
                part.spice_model,
                part.contact_config,
                part.plc_module,
            ],
        )?;
        Ok(())
    }

    pub fn get_part(&self, part_no: &str) -> Result<Option<Part>, PartsError> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {PART_COLUMNS} FROM parts WHERE part_no=?1"))?;
        let mut rows = stmt.query_map([part_no], row_to_part)?;
        Ok(rows.next().transpose()?)
    }

    /// 型番・名称・メーカの部分一致検索。categoryは完全一致で絞り込み。空クエリは全件。
    pub fn search_parts(&self, query: &str, category: Option<&str>) -> Result<Vec<Part>, PartsError> {
        let like = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {PART_COLUMNS} FROM parts \
             WHERE (part_no LIKE ?1 OR name LIKE ?1 OR maker LIKE ?1) \
             AND (?2 IS NULL OR category = ?2) \
             ORDER BY part_no"
        ))?;
        let rows = stmt.query_map(rusqlite::params![like, category], row_to_part)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// PLC I/Oモジュールの一覧 (`plc_module`列にモジュール定義が入っている部品だけ)。
    /// PLC I/O図面の生成で「どの機種を置くか」を選ぶモジュールライブラリになる。
    pub fn list_plc_modules(&self) -> Result<Vec<Part>, PartsError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {PART_COLUMNS} FROM parts WHERE plc_module <> '' ORDER BY part_no"
        ))?;
        let rows = stmt.query_map([], row_to_part)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete_part(&self, part_no: &str) -> Result<bool, PartsError> {
        Ok(self
            .conn
            .execute("DELETE FROM parts WHERE part_no=?1", [part_no])?
            > 0)
    }

    pub fn upsert_wire_part(&self, row: &WirePartRow) -> Result<(), PartsError> {
        self.conn.execute(
            "INSERT INTO wire_parts (part_no, color, sq, purchase_url, price_per_m, note) \
             VALUES (?1,?2,?3,?4,?5,?6) \
             ON CONFLICT(part_no) DO UPDATE SET \
             color=?2, sq=?3, purchase_url=?4, price_per_m=?5, note=?6",
            rusqlite::params![
                row.part_no,
                row.color,
                row.sq,
                row.purchase_url,
                row.price_per_m,
                row.note
            ],
        )?;
        Ok(())
    }

    pub fn list_wire_parts(&self) -> Result<Vec<WirePartRow>, PartsError> {
        let mut stmt = self.conn.prepare(
            "SELECT part_no, color, sq, purchase_url, price_per_m, note \
             FROM wire_parts ORDER BY part_no",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WirePartRow {
                part_no: row.get(0)?,
                color: row.get(1)?,
                sq: row.get(2)?,
                purchase_url: row.get(3)?,
                price_per_m: row.get(4)?,
                note: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// 線色+sqから品番を引く。
    pub fn find_wire_part(&self, color: &str, sq: f64) -> Result<Option<WirePartRow>, PartsError> {
        Ok(self
            .list_wire_parts()?
            .into_iter()
            .find(|w| w.color == color && (w.sq - sq).abs() < 1e-9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("madake-parts-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}-{}.sqlite", std::process::id()));
        std::fs::remove_file(&p).ok();
        p
    }

    /// Opening a new database creates the schema and seeds sample parts exactly once; reopening never re-seeds.
    /// 新規DBを開くとスキーマ作成とサンプル投入が一度だけ行われ、開き直しても再投入されない。
    #[test]
    fn open_creates_schema_and_seeds_samples_once() {
        let path = tmp_db("seed");
        let db = PartsDb::open(&path).unwrap();
        let all = db.search_parts("", None).unwrap();
        assert!(all.len() >= 5, "サンプル部品が入る: {}", all.len());
        assert!(!db.list_wire_parts().unwrap().is_empty(), "電線サンプルも入る");
        // 1件消して開き直してもサンプルは再投入されない
        let first = all[0].part_no.clone();
        assert!(db.delete_part(&first).unwrap());
        drop(db);
        let db = PartsDb::open(&path).unwrap();
        assert!(db.get_part(&first).unwrap().is_none(), "再投入されない");
        std::fs::remove_file(&path).ok();
    }

    /// Parts can be inserted, updated by part number, fetched, and searched by partial name match or exact category.
    /// 部品は登録・型番キーでの更新・取得ができ、名称の部分一致やカテゴリ完全一致で検索できる。
    #[test]
    fn upsert_get_and_search() {
        let path = tmp_db("crud");
        let db = PartsDb::open(&path).unwrap();
        let part = Part {
            part_no: "OMR-MY2N-D2-DC24".into(),
            maker: "オムロン".into(),
            name: "ミニパワーリレー 2極".into(),
            category: "relay".into(),
            symbol_id: "relay_coil".into(),
            rated_voltage: "DC24V".into(),
            rated_current_a: Some(0.05),
            purchase_url: "https://example.com/buy".into(),
            datasheet_url: "https://example.com/ds.pdf".into(),
            price: Some(880.0),
            currency: "JPY".into(),
            note: "テスト".into(),
            ..Default::default()
        };
        db.upsert_part(&part).unwrap();
        assert_eq!(db.get_part("OMR-MY2N-D2-DC24").unwrap().as_ref(), Some(&part));
        // 上書き
        let mut updated = part.clone();
        updated.price = Some(920.0);
        db.upsert_part(&updated).unwrap();
        assert_eq!(db.get_part("OMR-MY2N-D2-DC24").unwrap().unwrap().price, Some(920.0));
        // 検索: 名称部分一致 + カテゴリ絞り込み
        let hits = db.search_parts("ミニパワー", None).unwrap();
        assert_eq!(hits.len(), 1);
        let hits = db.search_parts("", Some("relay")).unwrap();
        assert!(hits.iter().any(|p| p.part_no == "OMR-MY2N-D2-DC24"));
        assert!(db.search_parts("存在しない部品", None).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    /// Wire parts are registered and looked up by exact color + gauge combination.
    /// 電線品番は登録でき、線色+線径の完全一致で引き当てられる。
    #[test]
    fn wire_parts_crud_and_lookup() {
        let path = tmp_db("wire");
        let db = PartsDb::open(&path).unwrap();
        // サンプルに無い 色+sq の組で登録・検索する
        db.upsert_wire_part(&WirePartRow {
            part_no: "TEST-W-125GN".into(),
            color: "green".into(),
            sq: 1.25,
            purchase_url: String::new(),
            price_per_m: Some(35.0),
            note: String::new(),
        })
        .unwrap();
        let hit = db.find_wire_part("green", 1.25).unwrap().unwrap();
        assert_eq!(hit.part_no, "TEST-W-125GN");
        assert!(db.find_wire_part("green", 2.0).unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    /// An old schema-v1 database migrates to v2 on open, preserving existing rows and gaining the spice_model column.
    /// 旧スキーマv1のDBは開いた時点でv2へ移行され、既存データを保持したままspice_model列が使えるようになる。
    #[test]
    fn v1_database_migrates_to_v2_preserving_data() {
        let path = tmp_db("migrate");
        // v1相当のDBを手で作る (spice_model列なし、version=1)
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '1');
                 CREATE TABLE parts (
                   part_no TEXT PRIMARY KEY,
                   maker TEXT NOT NULL DEFAULT '', name TEXT NOT NULL DEFAULT '',
                   category TEXT NOT NULL DEFAULT '', symbol_id TEXT NOT NULL DEFAULT '',
                   rated_voltage TEXT NOT NULL DEFAULT '', rated_current_a REAL,
                   purchase_url TEXT NOT NULL DEFAULT '', datasheet_url TEXT NOT NULL DEFAULT '',
                   price REAL, currency TEXT NOT NULL DEFAULT 'JPY',
                   note TEXT NOT NULL DEFAULT '', model_3d TEXT NOT NULL DEFAULT '',
                   mounting TEXT NOT NULL DEFAULT ''
                 );
                 INSERT INTO parts (part_no, name) VALUES ('OLD-1', '旧部品');
                 CREATE TABLE wire_parts (
                   part_no TEXT PRIMARY KEY, color TEXT NOT NULL, sq REAL NOT NULL,
                   purchase_url TEXT NOT NULL DEFAULT '', price_per_m REAL,
                   note TEXT NOT NULL DEFAULT ''
                 );",
            )
            .unwrap();
        }
        let db = PartsDb::open(&path).unwrap();
        // 既存データが残り、spice_modelは空文字で読める
        let old = db.get_part("OLD-1").unwrap().unwrap();
        assert_eq!(old.name, "旧部品");
        assert_eq!(old.spice_model, "");
        // spice_modelの書き込みも可能
        let mut updated = old.clone();
        updated.spice_model = "D1 {a} {k} DMOD".into();
        db.upsert_part(&updated).unwrap();
        assert_eq!(
            db.get_part("OLD-1").unwrap().unwrap().spice_model,
            "D1 {a} {k} DMOD"
        );
        // サンプルは再投入されない (v1で既にseed済みの想定)
        assert!(db.get_part("MDK-FUSE-5A").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    /// An old schema-v2 database migrates to v3 on open, preserving existing rows and gaining the contact_config column.
    /// 旧スキーマv2のDBは開いた時点でv3へ移行され、既存データを保持したままcontact_config列が使えるようになる。
    #[test]
    fn v2_database_migrates_to_v3_preserving_data() {
        let path = tmp_db("migrate-v3");
        // v2相当のDBを手で作る (contact_config列なし、version=2)
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '2');
                 CREATE TABLE parts (
                   part_no TEXT PRIMARY KEY,
                   maker TEXT NOT NULL DEFAULT '', name TEXT NOT NULL DEFAULT '',
                   category TEXT NOT NULL DEFAULT '', symbol_id TEXT NOT NULL DEFAULT '',
                   rated_voltage TEXT NOT NULL DEFAULT '', rated_current_a REAL,
                   purchase_url TEXT NOT NULL DEFAULT '', datasheet_url TEXT NOT NULL DEFAULT '',
                   price REAL, currency TEXT NOT NULL DEFAULT 'JPY',
                   note TEXT NOT NULL DEFAULT '', model_3d TEXT NOT NULL DEFAULT '',
                   mounting TEXT NOT NULL DEFAULT '', spice_model TEXT NOT NULL DEFAULT ''
                 );
                 INSERT INTO parts (part_no, name, spice_model) VALUES ('OLD-2', '旧部品', 'R1 a b 1k');
                 CREATE TABLE wire_parts (
                   part_no TEXT PRIMARY KEY, color TEXT NOT NULL, sq REAL NOT NULL,
                   purchase_url TEXT NOT NULL DEFAULT '', price_per_m REAL,
                   note TEXT NOT NULL DEFAULT ''
                 );",
            )
            .unwrap();
        }
        let db = PartsDb::open(&path).unwrap();
        // 既存データが残り、contact_configは空文字で読める
        let old = db.get_part("OLD-2").unwrap().unwrap();
        assert_eq!(old.name, "旧部品");
        assert_eq!(old.spice_model, "R1 a b 1k");
        assert_eq!(old.contact_config, "");
        // contact_configの書き込みも可能
        let mut updated = old.clone();
        updated.contact_config = "2NO+2NC".into();
        db.upsert_part(&updated).unwrap();
        assert_eq!(
            db.get_part("OLD-2").unwrap().unwrap().contact_config,
            "2NO+2NC"
        );
        // サンプルは再投入されない (v2で既にseed済みの想定)
        assert!(db.get_part("MDK-FUSE-5A").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    /// The bundled sample relay carries its contact configuration, so a freshly placed relay can be checked for contact overflow.
    /// 同梱のサンプルリレーは接点構成を持っているので、配置直後から接点数超過の検証ができる。
    #[test]
    fn sample_relay_part_has_a_contact_configuration() {
        let path = tmp_db("relay-config");
        let db = PartsDb::open(&path).unwrap();
        let relays = db.search_parts("", Some("relay")).unwrap();
        assert!(!relays.is_empty(), "リレーのサンプルがある");
        assert!(
            relays.iter().all(|p| !p.contact_config.is_empty()),
            "接点構成が入っている: {relays:?}"
        );
        std::fs::remove_file(&path).ok();
    }

    /// An old schema-v3 database migrates to v4 on open, preserving existing rows and gaining the plc_module column.
    /// 旧スキーマv3のDBは開いた時点でv4へ移行され、既存データを保持したままplc_module列が使えるようになる。
    #[test]
    fn v3_database_migrates_to_v4_preserving_data() {
        let path = tmp_db("migrate-v4");
        // v3相当のDBを手で作る (plc_module列なし、version=3)
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '3');
                 CREATE TABLE parts (
                   part_no TEXT PRIMARY KEY,
                   maker TEXT NOT NULL DEFAULT '', name TEXT NOT NULL DEFAULT '',
                   category TEXT NOT NULL DEFAULT '', symbol_id TEXT NOT NULL DEFAULT '',
                   rated_voltage TEXT NOT NULL DEFAULT '', rated_current_a REAL,
                   purchase_url TEXT NOT NULL DEFAULT '', datasheet_url TEXT NOT NULL DEFAULT '',
                   price REAL, currency TEXT NOT NULL DEFAULT 'JPY',
                   note TEXT NOT NULL DEFAULT '', model_3d TEXT NOT NULL DEFAULT '',
                   mounting TEXT NOT NULL DEFAULT '', spice_model TEXT NOT NULL DEFAULT '',
                   contact_config TEXT NOT NULL DEFAULT ''
                 );
                 INSERT INTO parts (part_no, name, contact_config) VALUES ('OLD-3', '旧部品', '2NO');
                 CREATE TABLE wire_parts (
                   part_no TEXT PRIMARY KEY, color TEXT NOT NULL, sq REAL NOT NULL,
                   purchase_url TEXT NOT NULL DEFAULT '', price_per_m REAL,
                   note TEXT NOT NULL DEFAULT ''
                 );",
            )
            .unwrap();
        }
        let db = PartsDb::open(&path).unwrap();
        let old = db.get_part("OLD-3").unwrap().unwrap();
        assert_eq!(old.name, "旧部品");
        assert_eq!(old.contact_config, "2NO");
        assert_eq!(old.plc_module, "", "PLCモジュールでない部品は空欄");
        // plc_moduleの書き込みも可能
        let mut updated = old.clone();
        updated.plc_module =
            r#"{"points":8,"kind":"DI","address_prefix":"X","address_style":"mitsubishi"}"#.into();
        db.upsert_part(&updated).unwrap();
        assert!(db
            .get_part("OLD-3")
            .unwrap()
            .unwrap()
            .plc_module
            .contains("\"points\":8"));
        // サンプルは再投入されない (v3で既にseed済みの想定)
        assert!(db.get_part("MDK-FUSE-5A").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    /// The bundled samples include three PLC modules that cover the Mitsubishi, Siemens and Allen-Bradley address styles.
    /// 同梱サンプルには三菱・Siemens・Allen-Bradleyのアドレス体系をひと通り含む3種のPLCモジュールが入っている。
    #[test]
    fn sample_plc_modules_cover_the_three_address_styles() {
        let path = tmp_db("plc-samples");
        let db = PartsDb::open(&path).unwrap();
        let modules = db.list_plc_modules().unwrap();
        assert!(modules.len() >= 3, "PLCモジュールのサンプル: {modules:?}");
        let specs: Vec<crate::plc::PlcModuleSpec> = modules
            .iter()
            .map(|p| crate::plc::PlcModuleSpec::parse(&p.plc_module).expect("読める定義"))
            .collect();
        assert!(specs.iter().any(|s| s.kind == crate::plc::PlcIoKind::Di && s.points == 16));
        assert!(specs.iter().any(|s| s.kind == crate::plc::PlcIoKind::Do && s.points == 16));
        assert!(specs
            .iter()
            .any(|s| s.address_style == crate::plc::PlcAddressStyle::Siemens && s.points == 8));
        // モジュールは点数に合った動的シンボルを既定に持つ
        for (part, spec) in modules.iter().zip(&specs) {
            assert_eq!(part.symbol_id, spec.symbol_id(), "{}", part.part_no);
        }
        std::fs::remove_file(&path).ok();
    }

    /// The PLC module list contains only parts that carry a module definition, so ordinary parts never show up in the module library.
    /// PLCモジュールの一覧はモジュール定義を持つ部品だけを返すので、普通の部品がモジュールライブラリに紛れ込まない。
    #[test]
    fn the_plc_module_list_contains_only_parts_with_a_module_definition() {
        let path = tmp_db("plc-only");
        let db = PartsDb::open(&path).unwrap();
        let modules = db.list_plc_modules().unwrap();
        assert!(modules.iter().all(|p| !p.plc_module.is_empty()));
        assert!(
            !modules.iter().any(|p| p.part_no == "MDK-FUSE-5A"),
            "PLCでない部品は出ない"
        );
        std::fs::remove_file(&path).ok();
    }

    /// The database path defaults to the OS app-data folder and can be overridden with MADAKE_PARTS_DB.
    /// DBパスの既定はOSのアプリデータフォルダで、MADAKE_PARTS_DBで上書きできる。
    #[test]
    fn default_path_respects_env_override() {
        // 環境変数が設定されていればそれを使う (プロセス全体に影響するのでキーは専用に)
        std::env::set_var("MADAKE_PARTS_DB", "/tmp/custom-parts.sqlite");
        assert_eq!(default_db_path(), PathBuf::from("/tmp/custom-parts.sqlite"));
        std::env::remove_var("MADAKE_PARTS_DB");
        assert!(default_db_path().ends_with("MadakeCAD/parts.sqlite"));
    }
}
