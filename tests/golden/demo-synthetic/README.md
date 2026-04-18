# Demo Synthetic Fixture

> 比賽 demo / 錄影 / regression 用的**合成 repo**，不是真實專案。
> 關聯：IA §13（demo 素材）、O-05 Sanitizer Diff（稽核畫面需要可預測的命中集）、D-008（Agent console demo 劇本）。

---

## 一、為什麼要 synthetic

Demo / 錄影時不能用使用者自己的 repo（隱私、不穩定、不可重現）。
所以準備一份**刻意植入 secret / PII / 內部識別符**的假 repo，特性：

- **完全合法的 fake value**（RFC 2606 `.example` TLD、RFC 5737 文件用 IP、`AKIAIOSFODNN7EXAMPLE` 等公認示範字串）
- **每次 scan 產出一致的 placeholder 集**（規則命中數可預期 → O-05 的 `matched: N` 數字穩定）
- **檔案數 / 目錄結構夠多樣**但**總量小**（< 50 檔），Scanner + KB build 秒回
- **語言跨 TS / Python / YAML / Markdown / .env**，覆蓋 MVP 主要掃描類型

---

## 二、目錄結構（規劃）

```
tests/golden/demo-synthetic/
├── README.md                 # 本檔
├── repo/                     # 假 repo root（scan 目標）
│   ├── src/
│   │   ├── adapters/
│   │   │   └── s3.ts         # AWS key + email + internal domain（O-05 主角）
│   │   ├── auth/
│   │   │   └── jwt.py        # JWT secret + DB 連線字串
│   │   └── main.ts
│   ├── config/
│   │   ├── auth.prod.env     # 多組 KEY=value
│   │   └── servers.yaml      # 內部 hostname
│   ├── README.md             # email、支援信箱
│   ├── package.json
│   └── .gitignore
└── expected/                 # 跑完 Sanitizer 的預期產物（golden）
    ├── placeholders.json     # 每檔期望的 placeholder_id 集合
    ├── rule_stats.json       # 每 rule 期望命中數（feed O-05 RIGHT pane）
    └── sanitize_audit.jsonl  # 期望的 audit log 逐行（不含原值）
```

---

## 三、植入的 fake value（白名單，不是真 secret）

| 類別 | 值 | 來源 |
|---|---|---|
| AWS access key | `AKIAIOSFODNN7EXAMPLE` | AWS 官方示範 |
| Secret key | `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` | AWS 官方示範 |
| Email | `*.example` TLD（RFC 2606） | `dev@timeline.example` 等 |
| 內部域名 | `*.timeline-internal.io` / `*.tl-corp.net` | 本 fixture 自訂 |
| 內部 IP | `192.0.2.0/24` / `198.51.100.0/24` / `203.0.113.0/24`（RFC 5737 TEST-NET） | 文件示範區段 |
| 台灣手機 | `0912-345-678`（無實主）| 自訂 |
| 身分證 | `A123456789`（格式正確，非真 ID） | 自訂 |
| JWT | 自產 HS256 token with fake claims | 自產 |

**原則**：所有植入值須在公認示範清單內，或**格式正確但保證無實主**。不得使用任何可能連到真實服務 / 真實人的值。

---

## 四、用途

### 1. Demo / 錄影
- 打開 CodeBus → 選 `tests/golden/demo-synthetic/repo/` → 掃完跳 O-05 → 觀眾看到「3 files sanitized, 14 placeholders, rule_stats 全綠 0 flagged」
- `src/adapters/s3.ts` 故意放最 dramatic 的組合（AWS key + email + internal domain 同檔），當 O-05 主秀檔案

### 2. Regression 測試
- CI 跑 Sanitizer Pass 1 → 對比 `expected/placeholders.json` 差異
- 任一 placeholder_id 漂移 → 強制人工 review（可能是規則改爛 / 誤殺 / 漏抓）
- `rule_stats` 數字漂移 → 代表計數邏輯壞掉

### 3. O-05 設計稿 mock 資料源
- `design/O-05 Sanitizer Diff.html` 的 mock `lines[]` 直接抄 `src/adapters/s3.ts`
- 未來若換 fixture 內容，mock 也同步改（單一真相）

---

## 五、實作順序

| 優先 | 項目 | 備註 |
|---|---|---|
| P0 | `repo/src/adapters/s3.ts`（O-05 主角檔） | 對齊目前 O-05 mock lines |
| P0 | `repo/config/auth.prod.env` | 多組 secret，驗 `.env` 規則 |
| P0 | `expected/placeholders.json` + `rule_stats.json` | 跑一次 Scanner 校準後凍結 |
| P1 | JWT / 身分證 / 手機 fixture | 驗 PII 類 |
| P1 | Golden regression 腳本（`scripts/check-sanitizer-golden.py`） | CI hook |
| P2 | 多語言覆蓋（Go / Rust 各一檔） | MVP 後補 |

---

## 六、MVP 明確不做

- 真實 repo mirror（`timeline` 等使用者專案）—— 只做合成
- Fake secret 自動生成器（手工挑選可控性更高）
- 跨平台路徑差異測試（Linux fixture for now，Windows 移植 MVP 後）
