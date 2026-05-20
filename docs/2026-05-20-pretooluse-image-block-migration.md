# 既有 vault 升級：加入 PreToolUse Read hook（pretooluse-image-block）

**Date:** 2026-05-20
**Related change:** `openspec/changes/pretooluse-image-block`
**Related spec:** `openspec/specs/lint-feedback-loop/spec.md` — Requirement `PII Image Read Hook Installation`

---

## 為什麼需要這個升級

`pretooluse-image-block` change 補上一條 PreToolUse Read hook，攔截 agent 對圖片 / binary 檔（PNG / JPG / PDF / GIF / WebP / BMP / TIFF / ICO / HEIC / HEIF / AVIF）的 Read 呼叫。這層 hook 是 codebus 「PII-sanitized wiki」核心保證的補強 —— `regex_basic` PII scanner 只掃**文字內容**，圖片 binary 走 Read tool 完全 bypass，agent 可把對圖片的觀察（含 credentials UI / 內網 dashboard / 個人臉孔）寫進 wiki。

新 init 的 vault 自動寫入兩條 hook entry（Bash + Read），**已 init 過的 vault 不會被自動 migrate**（`write_settings_if_missing` 對既有 `<vault>/.claude/settings.json` 維持 byte-identical 不覆寫契約）。已 init 的 user 須**擇一**手動處理。

## 方案 A：手動加 JSON snippet（推薦）

打開 `<repo>/.codebus/.claude/settings.json`（vault 內部），原本內容應該長這樣：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "codebus hook check-bash"
          }
        ]
      }
    ]
  }
}
```

在 `PreToolUse` 陣列**追加**第二個 entry，變成：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "codebus hook check-bash"
          }
        ]
      },
      {
        "matcher": "Read",
        "hooks": [
          {
            "type": "command",
            "command": "codebus hook check-read"
          }
        ]
      }
    ]
  }
}
```

存檔。下次 `codebus goal` / `query` / `chat` / `fix` / `quiz` spawn agent 就會自動套用新 hook。

驗證方式：直接從命令列跑 `codebus hook check-read`，貼下面這行 JSON 進 stdin 後按 Ctrl+Z（Windows）/ Ctrl+D（Unix）：

```
{"tool_name":"Read","tool_input":{"file_path":"foo.png"}}
```

預期 stdout 印出含 `"decision":"block"` 的 JSON，代表 hook 已生效。

## 方案 B：re-init（會丟失現有客製化 settings.json）

只在你**沒有**對 `<repo>/.codebus/.claude/settings.json` 做任何手動客製化時可用。步驟：

1. 刪掉 `<repo>/.codebus/.claude/settings.json`
2. 重新跑 `codebus init`（不會碰 wiki 內容，只會重建 settings.json）

`codebus init` 用 write-if-missing 寫入新版（兩條 entry 都在）。

不建議走這條，除非你確認沒手動改過 settings.json —— 因為 init 不會 diff merge，會用 default template 取代。

## 為什麼不自動 migrate

設計上保留 `write_settings_if_missing` 的 byte-identical 不覆寫契約：

- user 可能對 settings.json 加了自己的 PreToolUse hook chain 或其他 Claude Code 設定
- 自動 diff merge JSON 物件是非平凡工程（hook order / 重複 entry / 巢狀 array），且任何 merge 行為都有「破壞 user 客製化」的風險
- 走 release note 引導 user 手動操作，user 對 settings.json 的內容有完全掌控

未來如果出現 `codebus init --migrate-hooks` 子命令，會走獨立 propose / spec / change 流程，本 change 範圍不包含。

## Migration 不做也行嗎

可以，但**你的既有 vault 就持續暴露**這個 PII 洩漏路徑：agent Read 圖片 → 寫入 wiki → auto-commit 包進 nested git → user 提交 source repo 一起把 PII 帶出去。

如果你的 repo 確定沒任何圖片 / PDF / binary 檔，理論上 hook 沒事可做、不升級也沒實際 risk。但既然 hook 是 safety floor，升級了就是「無風險加倍保險」。
