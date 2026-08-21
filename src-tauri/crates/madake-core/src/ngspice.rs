//! ngspiceランナー (OS非依存)。サブプロセスで`ngspice -b`を実行しDC動作点を得る。
//!
//! 実行ファイル探索の優先順位: 環境変数 `MADAKE_NGSPICE` → PATH → OS別の既定パス。
//! 見つからない環境では呼び出し側(verify)がグラフ近似へフォールバックする。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, thiserror::Error)]
pub enum NgspiceError {
    #[error("ngspiceの実行に失敗しました: {0}")]
    Spawn(String),
    #[error("ngspiceがエラー終了しました: {0}")]
    Failed(String),
    #[error("一時ファイルの作成に失敗しました: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(windows)]
const EXE_NAMES: &[&str] = &["ngspice.exe", "ngspice_con.exe"];
#[cfg(not(windows))]
const EXE_NAMES: &[&str] = &["ngspice"];

/// OS別の既定インストール先。
fn default_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    {
        out.push(PathBuf::from("/opt/homebrew/bin/ngspice"));
        out.push(PathBuf::from("/usr/local/bin/ngspice"));
    }
    #[cfg(target_os = "linux")]
    {
        out.push(PathBuf::from("/usr/bin/ngspice"));
        out.push(PathBuf::from("/usr/local/bin/ngspice"));
    }
    #[cfg(windows)]
    {
        out.push(PathBuf::from(r"C:\Program Files\Spice64\bin\ngspice.exe"));
        out.push(PathBuf::from(r"C:\Spice64\bin\ngspice.exe"));
    }
    out
}

/// 探索ロジック本体 (テスト可能な純関数)。
fn find_in(
    env_override: Option<PathBuf>,
    path_dirs: &[PathBuf],
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(p) = env_override {
        if p.is_file() {
            return Some(p);
        }
    }
    for dir in path_dirs {
        for name in EXE_NAMES {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    candidates.iter().find(|p| p.is_file()).cloned()
}

/// ngspice実行ファイルを探す。
pub fn find_ngspice() -> Option<PathBuf> {
    let env_override = std::env::var_os("MADAKE_NGSPICE").map(PathBuf::from);
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    find_in(env_override, &path_dirs, &default_candidates())
}

/// `print all` 出力をパースする。ノード電圧は "v(n1) = 2.4e+01" → キー "n1"、
/// 枝電流は "v1#branch = -2.0e0" → キー "v1#branch"。
pub fn parse_print_all(output: &str) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    for line in output.lines() {
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim().to_ascii_lowercase();
        let Ok(value) = value.trim().parse::<f64>() else { continue };
        let key = name
            .strip_prefix("v(")
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(&name)
            .to_string();
        if key.is_empty() || key.contains(' ') {
            continue;
        }
        map.insert(key, value);
    }
    map
}

/// デッキを`.op`で実行し、ノード電圧(+枝電流)マップを返す。
pub fn run_op(exe: &Path, deck: &str) -> Result<BTreeMap<String, f64>, NgspiceError> {
    // 並列実行(テスト含む)で衝突しないよう、PID+連番で一意にする
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    let file = dir.join(format!(
        "madake-verify-{}-{}.cir",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let full = format!("{deck}.control\nop\nprint all\n.endc\n.end\n");
    std::fs::write(&file, full)?;
    let result = std::process::Command::new(exe)
        .arg("-b")
        .arg(&file)
        .output();
    std::fs::remove_file(&file).ok();
    let output = result.map_err(|e| NgspiceError::Spawn(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(NgspiceError::Failed(format!(
            "{}\n{}",
            stdout,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(parse_print_all(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_print_all_reads_nodes_and_branches() {
        let out = "\
No. of Data Rows : 1
v(n1) = 2.400000e+01
v(n2) = 2.399533e+01
v1#branch = -2.00000e+00
some noise line
";
        let map = parse_print_all(out);
        assert!((map["n1"] - 24.0).abs() < 1e-9);
        assert!((map["n2"] - 23.99533).abs() < 1e-5);
        assert!((map["v1#branch"] + 2.0).abs() < 1e-9);
        assert_eq!(map.len(), 3, "{map:?}");
    }

    #[test]
    fn find_in_prefers_env_then_path_then_candidates() {
        let dir = std::env::temp_dir().join(format!("madake-ng-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe_name = EXE_NAMES[0];
        let env_exe = dir.join("custom-ngspice");
        let path_exe = dir.join(exe_name);
        std::fs::write(&env_exe, "").unwrap();
        std::fs::write(&path_exe, "").unwrap();

        // env優先
        assert_eq!(
            find_in(Some(env_exe.clone()), &[dir.clone()], &[]),
            Some(env_exe.clone())
        );
        // envが無ければPATH
        assert_eq!(find_in(None, &[dir.clone()], &[]), Some(path_exe.clone()));
        // 存在しないenvは無視してPATHへ
        assert_eq!(
            find_in(Some(dir.join("missing")), &[dir.clone()], &[]),
            Some(path_exe.clone())
        );
        // どちらも無ければ既定パス
        assert_eq!(find_in(None, &[], &[path_exe.clone()]), Some(path_exe.clone()));
        assert_eq!(find_in(None, &[], &[dir.join("missing")]), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_op_solves_a_divider_when_ngspice_is_installed() {
        let Some(exe) = find_ngspice() else {
            eprintln!("ngspice未検出のためスキップ");
            return;
        };
        // 24V - 6Ω - 6Ω 分圧: 中点12V、電流2A
        let deck = "* divider\nV1 n1 0 DC 24\nR1 n1 n2 6\nR2 n2 0 6\n";
        let map = run_op(&exe, deck).expect("run_op");
        assert!((map["n1"] - 24.0).abs() < 1e-6, "{map:?}");
        assert!((map["n2"] - 12.0).abs() < 1e-6, "{map:?}");
        assert!((map["v1#branch"] + 2.0).abs() < 1e-6, "{map:?}");
    }
}
