# CodeBus Design Mockups

> **設計原件**收存處。實作以 `docs/*.md` spec 為準；若 mockup 與 spec 衝突，以 spec 為主、回頭修 mockup。

---

## 版本演進

| 版本 | 日期 | 範圍 | 說明 |
|---|---|---|---|
| **v0** | 2026-04-19 | Phase A Trust Layer 4 站初版 | 3 份 HTML + 14 張截圖；`O-01 三場景` 與 `O-05 LOCKED/UNLOCKED state machine` 兩個獨門特性留在這裡 |
| **v1** | 2026-04-27 | Phase A/B/C/D 全 8 大站完整版 | 14 mockup + 共用骨架（tokens / shell）+ design canvas |

---

## 一、Phase 6 動工指引

**v1 是主架構**：14 mockup + 共用骨架（tokens.css / shell.css / shell.js）對應 implementation-plan §六 步驟 26-30 全範圍。

**v0 是功能補強**，動工時參考兩個獨門特性：

| 特性 | 在哪裡 | 為何 v1 不夠 |
|---|---|---|
| **O-01 三場景**（first-run / new-kind / rules-bump） | [v0/o-01-grant-modal.html](v0/o-01-grant-modal.html) `:108-270` | v1 `03-grant.html` 只有單一場景。三場景變體是 `auth-flow` 動工不可少 — 對應 CLAUDE.md 不變式 #9（rules version bump 必須重授權） |
| **O-05 LOCKED/UNLOCKED state machine + 15min countdown** | [v0/o-05-sanitizer-diff.html](v0/o-05-sanitizer-diff.html) `:83-302` | v1 `14-sanitizer-diff.html` 是靜態畫面。unlock state machine 才是 audit unlock 真實 UX |

**v0 screenshots（14 張）** 是 visual reference + Demo 展示用，v1 沒有重渲染對應截圖。

---

## 二、v0 ↔ v1 對照（Trust Layer 四站）

| 站 | v0 | v1 | 動工取捨 |
|---|---|---|---|
| **O-01** Grant Modal | [v0/o-01-grant-modal.html](v0/o-01-grant-modal.html) + 3 張截圖 | [v1/03-grant.html](v1/03-grant.html) | v1 為 base + v0 補三場景互動 |
| **R-01** Workspace 主畫面 | [v0/r-01-workspace.html](v0/r-01-workspace.html) + 4 張截圖 | [v1/04-scan.html](v1/04-scan.html) + 共用骨架 | v1（拆得更乾淨，骨架抽出）|
| **O-04** LLM Call Inspector | （v0 只有 3 張截圖、無 HTML） | [v1/13-llm-call-inspector.html](v1/13-llm-call-inspector.html) | v1（v0 沒 HTML 可用）|
| **O-05** Sanitizer Diff | [v0/o-05-sanitizer-diff.html](v0/o-05-sanitizer-diff.html) + 4 張截圖 | [v1/14-sanitizer-diff.html](v1/14-sanitizer-diff.html) | v1 為 base + v0 補 unlock state machine |

---

## 三、Trust Layer 敘事串聯（v0 README 移過來）

```
R-01 主畫面
  ├─ 點 step → O-04 slide-in（看 LLM 真的吃到什麼）
  ├─ 點 sanitizer badge → O-05 稽核畫面（看原文 / 遮罩 / unlock 機制）
  └─ 首次開 workspace → O-01 modal（授權、選 scope / provider / ack）
```

四站一起構成「使用者可驗證原值沒外流」的信任鏈：
- **O-01** 定義邊界（哪個資料夾、哪個 LLM、哪些 kind 同意被處理）
- **O-05** 秀證據（原值在 LEFT 原樣、CENTER 只有 placeholder）
- **O-04** 揭露真相（LLM 實際收到的 wire payload）
- **R-01** 匯總（七層 audit JSONL 全可檢視）

---

## 四、v1 共用骨架（重點）

`v1/tokens.css` + `v1/shell.css` + `v1/shell.js` 是必須先 port 進 `web/` 的骨架，**不要每頁重寫 audit panel**：

- `tokens.css` → `tailwind.config.ts`（surface / accent / 字體）
- `shell.css` `.cb-app` / `.cb-split` / `.cb-stage` / `.cb-audit` → `layouts/default.vue`
- `shell.js::CB_TOPBAR` → `components/layout/TopBar.vue`
- `shell.js::CB_mountAudit` → `components/audit/AuditPanel.vue`（七 tab：sanitize / tool / reasoning / token / llm / kb_growth / generator，1:1 對應七層 JSONL）

⚠️ **`CB_AUDIT_SAMPLES` 是 mockup 假資料**，實作時必刪掉，改打 sidecar SSE 即時 push + tail 對應 jsonl。

詳見 [v1/README.md §二 共用骨架](v1/README.md) + §四 mockup vs 實作差異。

---

## 五、紅線（v1 README §四，與 CLAUDE.md 不變式 #2/#3/#5/#6 一致）

- ❌ 不可以把 pre-sanitize 原值落盤、寫 log、上傳
- ❌ 不可以 hardcode bearer / port —— 從 Tauri 啟動時 stdout handshake 拿
- ❌ 不可以跳過 `ensure_in_workspace` 直接讀檔
- ❌ Sanitizer placeholder 沒有反查表 —— UI 不要假裝可以還原

---

## 六、不在 repo 的原件

- `IA-standalone.html`（6.5MB，Claude Design 產出的互動式 IA 文件）
  - 列入 `design/.gitignore`，保留在本機開發環境
  - 需要時從 Claude Design 會話復原，或另外封存到 release artifact zip
  - 不納 repo 理由：體積會拖慢 clone、內容 spec 已另行拆成 `docs/ia.md`

---

## 七、更新規則

1. **Mockup 不是 spec**：實作時以 `docs/*.md` 為準；spec 改了要回頭修 mockup
2. **新版本汰換舊版本**：往後若有 v2，舊 v1 移到 `v1/` 不變、新版進 `v2/`，本 README §一 加 row
3. **檔名**：一律 kebab-case；v0 用 `{代號}-{語意}.html`（O-01 / R-01 / O-05 慣例），v1 用 `{順序}-{語意}.html`（design canvas 排序）
4. **新增畫面**：同步更新本 README §二 對照表
