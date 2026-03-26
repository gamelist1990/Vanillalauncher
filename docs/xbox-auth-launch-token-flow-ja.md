# Xbox 認証から Minecraft 起動までの技術詳細

## 1. 目的

このドキュメントは、VanillaLauncher が Windows 環境で Xbox 認証を確保し、Minecraft Java の起動に必要なアクセストークンを取得して、実際のゲーム起動引数へ反映するまでの内部フローを説明します。

主なゴールは次の 2 点です。

- `POST https://api.minecraftservices.com/launcher/login` を通して Minecraft Services の `access_token` を取得する
- 取得した認証情報を起動引数 (`auth_access_token` など) に埋め込んで Java プロセスを起動する

---

## 2. 全体フロー（高レベル）

1. React 側が起動前に `ensure_xbox_rps_state` を呼ぶ
2. Rust 側で保存済み state (`xbox-rps-last-success.json`) を優先して検証
3. state が古い/無効なら TokenBroker 候補を短距離探索し、成功候補を再保存
4. `launch_profile_directly` で `resolve_launch_auth` を実行
5. `launcher_accounts` の既存トークンが有効ならそれを採用
6. 無効なら TokenBroker から RPS チケットを取り出し、
   `user.auth -> xsts -> launcher/login` で `access_token` を取得
7. `minecraft/profile` でトークン有効性を確認
8. 起動引数の `auth_access_token` などに反映して Java を起動

---

## 3. 事前 state 管理（起動前最適化）

### 3.1 保存ファイル

保存先:

- `%TEMP%/VanillaLauncher/xbox-rps-last-success.json`

保存項目:

- `sourcePath`: 成功した `.tbres` の実パス
- `variantLabel`: 成功した変種ラベル（例: `from-t-no-prefix`）
- `relyingParty`: 利用した RP（通常 `rp://api.minecraftservices.com/`）
- `ticketPrefix`: トークン先頭の識別用プレビュー
- `expiresAt`: 候補期限
- `savedAt`: 保存時刻

### 3.2 自動再取得

次の条件では state をそのまま使わず、自動探索して更新します。

- state ファイルが存在しない
- `expiresAt` が過去
- `sourcePath`/`variantLabel` が現行候補に一致しない
- 保存候補で `launcher/login` まで成功しない

この処理は [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2614) の `ensure_xbox_rps_state` が担当します。

---

## 4. TokenBroker 候補の抽出

### 4.1 スキャン対象ディレクトリ

- `%LOCALAPPDATA%/Packages/Microsoft.XboxIdentityProvider_8wekyb3d8bbwe/AC/TokenBroker/Cache`
- `%LOCALAPPDATA%/Packages/Microsoft.GamingApp_8wekyb3d8bbwe/AC/TokenBroker/Cache`
- `%LOCALAPPDATA%/Microsoft/TokenBroker/Cache`

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2510)

### 4.2 解析手順

1. `.tbres` を UTF-16 JSON として読み込み
2. `ResponseBytes.Value` を取得
3. DPAPI (`CryptUnprotectData`) で復号
4. `WTRes_Token` を抽出
5. `t=` マーカーを基準に変種を生成
6. `Expiration` が期限切れなら除外

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2602)
- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2722)
- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L3032)

---

## 5. Xbox チェーンから launcher/login まで

### 5.1 RPS チケット交換

`exchange_rps_ticket_for_minecraft_access_token` で次を順に実行します。

1. `POST https://user.auth.xboxlive.com/user/authenticate`
2. `POST https://xsts.auth.xboxlive.com/xsts/authorize`
3. `XBL3.0 x=<uhs>;<xsts-token>` を構築
4. `POST https://api.minecraftservices.com/launcher/login`

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L3099)

### 5.2 launcher/login リトライ

`exchange_xbox_token_for_minecraft_access_token` では `429` 対策として再試行を実装しています。

- 最大 4 回
- `429` 時は増分待機

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L3032)

---

## 6. 起動時の認証解決

`launch_profile_directly` は `resolve_launch_auth` を通じて最終的な認証情報を決定します。

優先順位:

1. `launcher_accounts` 内の既存 `accessToken`（有効なら採用）
2. Xbox cache 由来の RPS チェーンで新規取得した `access_token`
3. 失敗時はフォールバック（公式ランチャー誘導を含む既存挙動）

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2327)
- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L904)

---

## 7. Minecraft 起動引数への反映

取得した認証情報は `build_launch_arguments` で置換変数へ注入されます。

主な置換キー:

- `auth_player_name`
- `auth_uuid`
- `auth_access_token`
- `auth_xuid`
- `user_properties`
- `auth_session`

これにより、Java 実行時に Minecraft が必要とする認証コンテキストが渡されます。

実装:

- [src-tauri/src/loaders.rs](src-tauri/src/loaders.rs#L2128)

---

## 8. React 連携ポイント

React 側では、直接起動前に state 確認を呼ぶ構成です。

- API 定義: [src/app/api.ts](src/app/api.ts)
- 型定義: [src/app/types.ts](src/app/types.ts)
- 起動導線: [src/App.tsx](src/App.tsx#L760)

これにより、起動ボタン押下時に「保存済み state の活用 -> 無効なら自動再取得」が毎回実行されます。

---

## 9. 実運用上のポイント

- 初回は state が空のため探索が走る
- 成功後は保存 state を最優先利用し、通常は短時間で成功しやすい
- 期限切れやキャッシュ更新で失敗しても、自動再探索で自己回復する
- サーバー混雑時は `429` が出るため、短い再試行と待機が必須

---

## 10. まとめ

VanillaLauncher は次の仕組みで、Xbox 認証を Minecraft 起動へ接続しています。

- Windows TokenBroker から RPS 由来候補を抽出
- `user.auth -> xsts -> launcher/login` で Minecraft `access_token` を獲得
- `minecraft/profile` で有効性を確認
- 起動引数へ `auth_access_token` 等を埋め込んで Java 起動

さらに state ファイルを使った再利用戦略により、毎回の長期試行を避けつつ、古い state の自動更新まで行う構成になっています。
