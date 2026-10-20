# OSS公開 手順書 (release runbook)

> **この文書に書かれた操作は実行されていない。** リポジトリの公開・GitHubへのpush・リモート作成・
> タグ付け・リリース発行はいずれも外向きの不可逆操作であり、**ユーザー本人が判断して実行する**。
> エージェントはこの手順書を書くところまでで止まる (M6計画 Task 5 Step 1)。
>
> 対応する仕様: [`specs/m6-oss.md`](specs/m6-oss.md) / 監査記録: [`release-audit.md`](release-audit.md)
>
> English: this runbook is Japanese-primary; the English summary is in [§8](#8-english-summary).

## 0. 前提 (公開前に済んでいること)

| 事項 | 確認方法 |
|---|---|
| ライセンス | `LICENSE-MIT` / `LICENSE-APACHE` が存在し、`README` とCargo/npmのメタデータが `MIT OR Apache-2.0` |
| コミュニティ文書 | `CONTRIBUTING.md`(+`.ja`)・`CODE_OF_CONDUCT.md`・`SECURITY.md`・`.github/ISSUE_TEMPLATE/`・`.github/PULL_REQUEST_TEMPLATE.md` |
| 依存ライセンス | [`third-party-licenses.md`](third-party-licenses.md) が最新 (`python3 scripts/gen_licenses.py`) |
| テスト | `cargo test --workspace` / `npx vitest run` / `npx vue-tsc --noEmit` / `python3 -m unittest discover -s freecad-addon/tests` / `python3 scripts/gen_spec.py --check` が全て緑 |
| 公開方法の決定 | 現リポジトリをそのまま公開するか、**新規リポジトリへスカッシュエクスポート**するか (監査記録 §1.5 の推奨は後者) |

## 1. 機密の最終確認 (最初にやる・ここで止まったら先へ進まない)

```bash
python3 scripts/audit_confidential.py --tree   # 作業ツリーのみ
python3 scripts/audit_confidential.py          # 作業ツリー + 全コミット履歴
```

- 用語表 `docs/references/confidential-terms.txt` は**git管理外**。各自ローカルに保持する
  (無いとスクリプトは即座に終了する)。だからCIには載せられない = **この手順1が唯一のゲート**
- 作業ツリーにヒットが残っていたら一般名へ置き換える (実型番・ネット名・企業名は全て対象)
- **履歴にヒットが残っていた場合は履歴を書き換えない。** 公開用に新規リポジトリへ
  スカッシュエクスポートする (下記 §2 の B 案)。仕様 §2 の方針
- `MadakeCAD.pen` (デザインファイル) も監査対象。中身は暗号化されているので
  Pen.app を起動して pencil MCP 経由で直す

## 2. 公開するリポジトリを決める

| 案 | 手順 | 向く場合 |
|---|---|---|
| **A: 現リポジトリをそのまま公開** | GitHubでリポジトリを作り、`git remote add origin …` → `git push` | 履歴に機密ヒットが0件 |
| **B: 新規リポジトリへスカッシュエクスポート**(監査記録の推奨) | 公開したいツリーだけを新しい空リポジトリの最初のコミットにする。元リポジトリは非公開のまま残す | 履歴にヒットがある / 履歴を公開したくない |

B案の要点: 作業ツリーを別ディレクトリへコピー → `.git` を削除 → `git init` →
`git add .` → 1コミット → 新リポジトリへpush。`.gitignore` が効くことを
`git status --short` と `git ls-files docs/references` (0件であること) で確かめる。

## 3. リポジトリの設定 (GitHub Web UI)

1. **可視性を Public にする** — 不可逆。手順1が緑であることをもう一度確かめる
2. **Description**: 産業用電気図面CAD (JIS準拠) / AIファースト作図。英語で1行
3. **Topics**: `cad`, `electrical`, `electrical-engineering`, `schematic`, `jis`,
   `tauri`, `vue`, `rust`, `dxf`, `kicad`, `freecad`, `mcp`, `ai`
4. **Features**: Issues 有効 / Discussions 有効 (質問の受け口) / Wiki 無効 (docsがあるため) /
   Projects は任意
5. **Security**: 「Private vulnerability reporting」を有効にする
   (`SECURITY.md` と `CODE_OF_CONDUCT.md:39` がこの窓口を前提にしている)
6. **Branch protection** (`master`): PR必須 + CI必須チェック (`Rust tests`,
   `Rust tests (…)`, `Frontend tests & types`, `FreeCAD add-on tests`,
   `Spec docs freshness`, `Bundle build (…)`)
7. **Actions**: 既定のまま (`ci.yml` は push/PR、`release.yml` はリリースの公開と手動)

## 4. 最初のリリース (タグと配布物)

1. ローカルで `npm run tauri build` が通ることを確認 (少なくとも手元のOSで)
2. GitHub の Releases →「Draft a new release」で新しいタグ `v0.1.0` (数字だけの `vX.Y.Z`。
   Windows の `.msi` がそれ以外を受け付けない) を作り、リリースノートを書く。最初は
   「Set as a pre-release」を付けておく。`tauri.conf.json` の `version` はタグから
   ランナー上で書き込まれるので、事前のバージョン上げコミットは要らない
3. リリースノートには最低限次を書く:
   - アルファ版であること・対応OS
   - **配布物は Developer ID 署名/notarization されていない** (macOSは「開発元を確認できません」、
     Windowsは SmartScreen の警告が出る。回避手順を1行添える。macOSは「システム設定 >
     プライバシーとセキュリティ」の「このまま開く」)。macOSのアプリは ad-hoc 署名だけ付く
     (`tauri.conf.json` の `bundle.macOS.signingIdentity: "-"`)。v0.3.0 までは ad-hoc 署名も
     無く、Apple silicon では「壊れているため開けません」になって開けなかった。その版を
     入れた人には `xattr -dr com.apple.quarantine /Applications/MadakeCAD.app` を案内する
   - **Linuxの`.deb`利用者はPDF出力の日本語書体に`fonts-noto-cjk`が必要** (無いと豆腐になる)
   - 既知の未実装 (`docs/12-roadmap.md` へのリンク)
4.「Publish release」すると `release.yml` が macOS (Apple silicon / Intel `.dmg`)・
   Windows (`.msi` / NSIS `.exe`)・Linux (`.AppImage` / `.deb` / `.rpm`) をビルドし、
   そのリリースへ添付する (20〜30分)。下書きのままでは GitHub がワークフローを起動しない
5. 配布物を作り直すときは Actions → Release → Run workflow にタグを入れて実行する

### 4.1 アプリ内アップデートの署名鍵 (最初のリリースの前に1回だけ)

インストール済みのアプリは起動時に `https://github.com/kousuke-takeuchi/MadakeCAD/releases/latest/download/latest.json`
を見て新しい版を知らせ、更新ファイルの**署名**をアプリに埋め込んだ公開鍵で確かめてから入れ替える
(tauri-plugin-updater)。鍵は利用者の手元で作り、秘密鍵はリポジトリに入れない。

1. 鍵を作る (パスワードを聞かれる。空にしない):
   `npx tauri signer generate -w ~/.tauri/madakecad.key`
   → `~/.tauri/madakecad.key` (秘密鍵) と `~/.tauri/madakecad.key.pub` (公開鍵) ができる
2. 公開鍵 (`.pub` の中身1行) を `src-tauri/tauri.conf.json` の `plugins.updater.pubkey` に入れてコミットする
3. GitHub の Settings → Secrets and variables → Actions に2つ登録する:
   `TAURI_SIGNING_PRIVATE_KEY` = 秘密鍵ファイルの中身、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 1. のパスワード
4. 秘密鍵とパスワードはパスワード管理ツール等にも保管する。**失くすと、配布済みのアプリへ更新を届けられなくなる**
   (新しい鍵で署名した更新は古いアプリが受け付けない)

秘密鍵が未登録のままリリースしても、インストーラは作られる (更新ファイルと `latest.json` が付かないだけ)。
秘密鍵を登録したのに公開鍵が空なら、`release.yml` は止まる。**プレリリースは「最新」に含まれない**ので、
更新として配られるのは通常のリリースだけ。リポジトリが非公開のままだと、利用者のアプリは `latest.json`
を取得できない (公開後に有効になる)。

## 5. 公開直後の初動 (最初の1週間)

| いつ | やること |
|---|---|
| 公開直後 | 自分でクリーンクローン → `docs/02-getting-started.md` どおりにビルド。手順が違っていたら即修正 |
| 公開直後 | Issue/PRテンプレートが表示されるか、Discussions が開いているかを実際に見る |
| 随時 | **初回応答は72時間以内**を目安にする (`CONTRIBUTING.md` の約束と齟齬が無いように) |
| 脆弱性報告 | Private vulnerability reporting 経由のみで扱う。公開Issueに書かれた場合は非公開へ誘導してから消す |
| 行動規範違反 | `CODE_OF_CONDUCT.md` の窓口 (private security advisory) で受ける |
| 1週間後 | Issueの傾向を `docs/12-roadmap.md` / `feature-inventory.md` に反映するか判断 |

## 6. 公開後に残る課題 (このリリースの範囲外)

- コード署名 / notarization (M6仕様 §3。macOS notarization を優先。macOSは ad-hoc 署名のみ済み)。アプリ内アップデートは実装済み (§4.1 の鍵が前提)
- `.deb` の依存に `fonts-noto-cjk` を入れる (現状はビルドマシンにしか入っていない)
- zh/es/fr/de カタログの翻訳の埋め (現状は約6割。[§7](#7-未決事項) )
- `.mcp.json` / `.claude/` を同梱し続けるかの判断 (社外の人には意味の無いローカルパスを含む)

## 7. 未決事項 (ユーザー判断)

| 事項 | 選択肢 |
|---|---|
| 公開方法 | A: 現リポジトリ / B: 新規リポジトリへスカッシュエクスポート |
| 公開名 | `MadakeCAD` のまま / 変更 |
| 公開時期 | 最初のタグと同時 / ソース公開を先行 |
| 翻訳 | zh/es/fr/de を埋めてから公開 / 部分対応と明記して公開 (現状の README はこちら) |

## 8. English summary

This runbook lists the steps **the user performs by hand**; none of them have been executed.

1. Run `python3 scripts/audit_confidential.py` (tree **and** history) first — the term list is
   git-ignored, so this local run is the only hygiene gate. Do not rewrite history; if history
   has hits, export a squashed snapshot into a fresh repository instead.
2. Decide between publishing this repository as-is and exporting a squashed snapshot.
3. Configure the repository: visibility → public, description, topics, Issues/Discussions,
   private vulnerability reporting, branch protection with the CI checks as required.
4. Cut the first release: draft a GitHub release with a new `vX.Y.Z` tag (numbers only), write
   the notes (unsigned bundles, `fonts-noto-cjk` for the `.deb`), then publish it; `release.yml`
   stamps the tag's version, builds the installers on all three platforms and attaches them to
   that release.
5. Before the first release, create the updater signing key (`npx tauri signer generate -w
   ~/.tauri/madakecad.key`), put the public key in `plugins.updater.pubkey` of
   `src-tauri/tauri.conf.json`, and store the private key and its password as the repository
   secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Keep a backup:
   losing the key means installed apps can no longer be updated.
6. First week: clean-clone build check, 72-hour first-response target, vulnerability reports
   through the private channel only.
