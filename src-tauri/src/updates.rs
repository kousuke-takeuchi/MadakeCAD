//! アプリ内アップデート (tauri-plugin-updater) をこの実行環境で使えるかの判定。
//!
//! 確認・ダウンロード・入れ替えはプラグインのJS API (`@tauri-apps/plugin-updater`) が行う。
//! ここはその前に「自動で入れ替えられる入れ方か」だけを答える:
//! - 開発ビルド (`npm run tauri dev`) は確認しない (手元のビルドを配布版で上書きしない)
//! - Linuxで自動更新できるのはAppImageだけ。`.deb` / `.rpm` で入れたものはパッケージ管理の
//!   持ち物なので、新しい版を知らせてリリースページを開くだけにする

use serde::Serialize;

/// この実行環境でのアップデートの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSupport {
    /// 確認して、署名を確かめてから入れ替え・再起動まで行える。
    Supported,
    /// 新しい版を知らせるだけ (入れ替えは利用者がリリースページから)。
    Manual,
    /// 開発ビルド。確認しない。
    Dev,
}

/// 判定の本体。`os`は`std::env::consts::OS`、`appimage`はAppImageから起動したか (環境変数`APPIMAGE`)。
pub fn update_support(debug_build: bool, os: &str, appimage: bool) -> UpdateSupport {
    if debug_build {
        UpdateSupport::Dev
    } else if os == "linux" && !appimage {
        UpdateSupport::Manual
    } else {
        UpdateSupport::Supported
    }
}

/// いま動いているアプリのアップデートの扱い。
#[tauri::command]
pub fn update_support_info() -> UpdateSupport {
    update_support(
        cfg!(debug_assertions),
        std::env::consts::OS,
        std::env::var_os("APPIMAGE").is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Installed release builds on macOS and Windows, and the Linux AppImage, can download and install an update themselves.
    /// macOS・Windowsの配布版とLinuxのAppImageは、アップデートを自分でダウンロードして入れ替えられる。
    #[test]
    fn release_builds_and_the_appimage_update_themselves() {
        assert_eq!(update_support(false, "macos", false), UpdateSupport::Supported);
        assert_eq!(update_support(false, "windows", false), UpdateSupport::Supported);
        assert_eq!(update_support(false, "linux", true), UpdateSupport::Supported);
    }

    /// A Linux install from the .deb or .rpm belongs to the package manager, so the app only announces the new version.
    /// Linuxで.deb/.rpmから入れたものはパッケージ管理の持ち物なので、アプリは新しい版を知らせるだけにする。
    #[test]
    fn a_deb_or_rpm_install_only_announces_the_update() {
        assert_eq!(update_support(false, "linux", false), UpdateSupport::Manual);
    }

    /// A development build never checks for updates, so a local build is never replaced by the released one.
    /// 開発ビルドはアップデートを確認しない (手元のビルドを配布版で上書きしない)。
    #[test]
    fn a_development_build_never_checks() {
        assert_eq!(update_support(true, "macos", false), UpdateSupport::Dev);
        assert_eq!(update_support(true, "linux", true), UpdateSupport::Dev);
    }
}
