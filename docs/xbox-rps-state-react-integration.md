# Xbox RPS State 連携実装（React + Tauri）

## 目的

本実装の目的は、毎回長時間の総当たり試行を行わずに、`/launcher/login` を短時間で成功させることです。

そのために以下を実現しています。

- React 側の起動導線から、起動直前に Xbox RPS state を必ず確認する
- 前回成功した候補を `xbox-rps-last-success.json` へ保存し、次回最優先で再利用する
- 保存済み state が古い、または無効化された場合は自動的に再探索して再保存する

---

## 全体アーキテクチャ

### 1. フロントエンド（React）

- 起動ボタン押下時に `ensure_xbox_rps_state` を先に呼び出す
- その後 `launch_profile_directly` を実行する

### 2. バックエンド（Tauri / Rust）

- `ensure_xbox_rps_state` コマンドを追加
- TokenBroker から候補を収集
- 保存済み state が有効なら最優先で使用
- 失敗時は上位候補を短い上限付きで再探索
- 成功時に state を更新保存

### 3. State ファイル

保存先:

- `%TEMP%/VanillaLauncher/xbox-rps-last-success.json`

格納内容:

- `sourcePath`: 成功した `.tbres` の実体パス
- `variantLabel`: 成功したトークン変種ラベル
- `relyingParty`: 使用した RP
- `ticketPrefix`: トークン先頭プレビュー
- `expiresAt`: 候補の有効期限
- `savedAt`: 保存時刻

---

## React 側の実装詳細

### 起動前の state 確認

`App.tsx` の直接起動処理で以下順序に変更。

1. `ensureXboxRpsState()` を実行
2. `launchProfileDirectly()` を実行

失敗時は即クラッシュさせず、ユーザー通知を出しつつ既存の起動処理へ進める設計です。

意図:

- state 更新が一時的に失敗しても、既存のランチャー起動ロジックを阻害しない
- ただし通常時は state がウォームされ、次回成功率を上げる

---

## Rust 側の実装詳細

### 追加コマンド

- `ensure_xbox_rps_state`（Tauri command）

戻り値:

- `XboxRpsStateResult`
  - `usedSavedState`
  - `refreshed`
  - `succeeded`
  - `sourcePath`
  - `variantLabel`
  - `statePath`
  - `message`

### 動作フロー

1. `read_cached_xbox_identity_tokens()` で有効候補を収集
2. `xbox-rps-last-success.json` を読み込み
3. `expiresAt` が有効で、`sourcePath + variantLabel` が現行候補に存在する場合は最優先投入
4. それでも失敗した場合、短い上限（12件）で再探索
5. 各候補に対して:
   - `exchange_rps_ticket_for_minecraft_access_token()` を試行
   - これは内部で `user.auth -> xsts -> /launcher/login` を実施
6. 成功したら state を即更新保存

### 探索コスト抑制

- 上限件数を固定（長期トライ回避）
- 候補スコアリングを導入
- 各試行間で短い待機を入れて `429` を抑制

---

## スコアリング戦略（概要）

高スコア優先で短距離探索します。

- `WA_UserName` 系を優先
- `from-t-marker` 系を優先
- 既知で成功しやすいプレフィックスを加点
- 既知で失敗しやすいプレフィックスを減点
- 保存済み成功 state は大幅加点（最優先）

これにより、初回ウォーム後は通常 1〜数試行で成功しやすくなります。

---

## 自動再取得・再保存の条件

次の条件で再取得が走ります。

- state ファイルが存在しない
- `expiresAt` が過去である
- `sourcePath` が現在の TokenBroker から消えた
- `variantLabel` が現行変種として解釈できない
- 既存 state で実行しても成功しない

再取得が成功すれば、同じファイルへ最新 state を上書き保存します。

---

## 運用コマンド

既定の検証コマンド:

- `bun run auth:probe:rps`

既定設定は短距離探索向けです。

- `--max-attempts 12 --delay 700`

---

## 期待される効果

- 毎回の長時間探索を避ける
- 初回成功後は保存 state により短時間で再成功しやすい
- state 劣化時も自動再探索で自己回復できる

---

## 補足

この設計は「成功候補を学習して再利用する」キャッシュ戦略です。

- ネットワーク状態
- サーバー側レート制限
- Windows 側トークン更新タイミング

に依存するため、常に 100% 同一結果を保証するものではありません。
ただし、長期トライ前提の実装より、実運用の応答性と再現性が大きく改善します。
