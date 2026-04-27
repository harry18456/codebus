# Design Mockups · Phase A Trust Layer

> Claude Design 產出的 HTML mockup + 對應截圖。**這些是設計原件**，實作以 `docs/` spec 為準；若兩者衝突，以 spec 為主、回頭修 mockup。

---

## 一、Mockup ↔ Spec ↔ Screenshot 對照

| 代號 | 畫面 | HTML | Spec | Screenshots |
|---|---|---|---|---|
| **R-01** | Workspace（主畫面 split view + 六層 audit） | [r-01-workspace.html](r-01-workspace.html) | `docs/ia.md` §R-01 / `docs/audit.md` | `screenshots/r-01-*.png` ×4 |
| **O-04** | LLM Call Inspector（R-01 內 slide-in panel） | 含於 `r-01-workspace.html` | `docs/reasoning-replay.md` / `docs/sidecar-api.md` §reasoning | `screenshots/o-04-*.png` ×3 |
| **O-05** | Sanitizer Diff（LOCKED / UNLOCKED 稽核畫面） | [o-05-sanitizer-diff.html](o-05-sanitizer-diff.html) | `docs/sanitizer.md` / `docs/sidecar-api.md` §audit/sanitize/diff | `screenshots/o-05-*.png` ×4 |
| **O-01** | Grant Modal（workspace 授權） | [o-01-grant-modal.html](o-01-grant-modal.html) | `docs/authorization.md` | `screenshots/o-01-*.png` ×3 |

---

## 二、Trust Layer 敘事串聯

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
- **R-01** 匯總（六層 audit JSONL 全可檢視）

---

## 三、Screenshot 清單（14 張）

由 Harry 本機渲染 HTML 後截圖，非 Claude Design 產出。

### R-01 Workspace（4 張）
- `r-01-overview.png` — 整體 split view（主畫面 + console + KB 預覽）
- `r-01-step-06-expanded.png` — step #06 被點擊展開成 O-04 panel
- `r-01-step-error-path-escape.png` — step error（path escape 被 ToolSandbox 擋下）
- `r-01-session-widget-hover.png` — session widget hover 態

### O-04 LLM Call Inspector（3 張）
- `o-04-wire-payload.png` — 預設態看 wire payload（placeholder 已替換原值）
- `o-04-step-context-strip.png` — step context strip（上游 decision 鏈）
- `o-04-pitch-banner.png` — pitch banner（「這才是 LLM 真的看到的」）

### O-05 Sanitizer Diff（4 張）
- `o-05-locked.png` — LOCKED 全景（raw values hidden）
- `o-05-unlocked.png` — UNLOCKED 全景（in-scope 還原）
- `o-05-unlock-modal.png` — Unlock Modal（15 分鐘 timeout 提示）
- `o-05-card-expanded.png` — 右側 rule card 展開 + regex toggle

### O-01 Grant Modal（3 張）
- `o-01-scenario-a-first-run.png` — 場景 a：first time workspace 授權
- `o-01-scenario-b-plus-new-kind.png` — 場景 b+：scope 升級（新 kind 🟣 biometric_id）
- `o-01-scenario-c-rules-bump.png` — 場景 c：sanitizer rules MAJOR bump 需重新授權

---

## 四、不在 repo 的原件

- `IA-standalone.html`（6.5MB，Claude Design 產出的互動式 IA 文件）
  - 列入 `design/.gitignore`，保留在本機開發環境
  - 需要時從 Claude Design 會話復原，或另外封存到 release artifact zip
  - 不納 repo 理由：體積會拖慢 clone、內容 spec 已另行拆成 `docs/ia.md`

---

## 五、更新規則

1. **Mockup 不是 spec**：實作時以 `docs/*.md` 為準；spec 改了要回頭修 mockup + screenshot
2. **版本汰換**：舊版（如 `R-01 Workspace.html` v1）直接刪除，不保留 `*-v1.html`；歷史去 git log 找
3. **檔名**：一律 kebab-case，格式 `{代號}-{語意}.html` / `{代號}-{語意}.png`
4. **新增畫面**：同步更新本 README §一 ↔ §三 對照表
