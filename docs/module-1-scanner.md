# Module 1 — Folder Scanner Spec

> 掃描 workspace、產出結構化檔案清單與 git metadata，餵給 Module 2 KB Builder。
> 關聯決策：D-015（Sanitizer）、D-016（Q&A 需 git metadata）、D-017（Sandbox）。
> 關聯文件：`sanitizer.md`、`tool-sandbox.md`、`sidecar-api.md` §三 `/scan`。

> **目前實作階段：`scanner-skeleton` + `scanner-sanitizer-orchestration`**
> 本 spec 描述的是 Scanner 的最終目標形狀。骨架階段實作「檔案遍歷 +
> 類型判別 + 編碼偵測 + 語言識別 + content-type summary + 同步 `/scan` endpoint」；
> `scanner-sanitizer-orchestration` change 接續補上 Pass 1 Sanitizer 串接與 quarantine。
> 以下子系統仍以 **stub 欄位 / 不動作** 形式先於 schema 預留，等後續 change 再實作：
>
> - **Git metadata（§八）**：`ScanResult.git` 永遠為 `None`，不開 `.git/`、不呼 pygit2。
> - **Monorepo 偵測（§七）**：`is_monorepo=False`、`monorepo_type=None`、`sub_packages=[]`，不解析 workspace 設定檔。
> - **SSE Progress（§十三）**：`/scan` 目前為同步單 body JSON，無 `progress` event。

---

## 一、職責與邊界

> **Skeleton 範圍註記**：下方「負責」項目中
> *Sanitize orchestration* / *Monorepo 子模組識別* / *Git metadata 收集* 骨架不實作，
> 只保留 schema 欄位；*Content-type summary* 已實作（含 `has_tests` / `has_docs` /
> `dominant_category` / `dominant_languages`）。「不負責」邊界仍然有效。

### 負責
- 檔案遍歷（遵守 `.gitignore` / `.dockerignore` / 使用者 config）
- 類型判別（text / binary / 超大檔 / generated）
- 編碼偵測與解碼（utf-8 → big5 → gbk fallback）
- 語言識別（靠副檔名 + 內容 heuristic）
- Monorepo 子模組識別
- Git metadata 收集（log / blame / 活動度）
- **Sanitize orchestration**（每個文字檔過 Sanitizer 第一段）
- **Content-type summary** 產出（repo 性質速覽，Explorer 啟動時用）
- 產出結構化 JSON 給 Module 2

### 不負責
- 向量化 / embedding（那是 Module 2）
- 內容理解 / 語意分析（那是 Module 4 Explorer）
- Sanitizer 規則細節（`sanitizer.md` 定義）
- 路徑安全檢查（`tool-sandbox.md` 定義，Scanner 使用其 helper）

---

## 二、資料流

```
workspace_root
    │
    ▼
[A] 枚舉檔案（遞迴 + gitignore 過濾 + symlink 防護）
    │
    ▼
[B] 逐檔分類（binary / text / 超大 / generated）
    │
    ├─▶ Binary / 超大 / generated：只記 metadata，不讀內容
    │
    └─▶ Text：
         ├─▶ [C] 編碼偵測與解碼
         ├─▶ [D] 語言識別
         ├─▶ [E] **Sanitizer 第一段**（D-015）
         └─▶ 記 {path, clean_content, language, encoding}
    │
    ▼
[F] Git metadata 收集（pygit2）
    │
    ▼
[G] 輸出 ScanResult JSON
```

---

## 三、檔案遍歷與過濾

> **Skeleton 範圍註記**：僅實作 `.gitignore` 階層疊加 + 內建 always-ignore +
> symlink 不跟隨（記 `Symlink` entry）；`.dockerignore` / `.codebusignore` /
> `follow_symlinks: true` config 尚未實作。並行策略目前是「遍歷單執行緒、內容
> 解碼亦單執行緒同步進行」，`asyncio.gather` 分批留待後續 change 處理。

### 遍歷方式
- **基礎**：`pathlib.Path.rglob("*")` 自 workspace_root 起
- **每個 entry 先過 `ensure_in_workspace()`**（`tool-sandbox.md` §三）
- **遇目錄**：檢查 gitignore；若命中則整棵 subtree skip（不 rglob 進去）
- **遇檔案**：進下游分類

### 符號連結處理（與 Sandbox 一致）
- **預設不跟隨**（`Path.rglob` 預設行為即是）
- 遇 symlink：記在 `symlinks[]`，**不 follow**
- 若使用者 config `follow_symlinks: true` 才跟隨，並在 `ensure_in_workspace` 驗證 resolve 後仍在 workspace 內

### gitignore 繼承規則

用 `pathspec` library（Python，標準 gitignore 語法）：

```python
from pathspec import PathSpec

# 階層式：每進一層目錄，疊加該目錄的 .gitignore
def build_ignore_for(dir_path: Path, parent_spec: PathSpec) -> PathSpec:
    local = dir_path / ".gitignore"
    if local.exists():
        lines = local.read_text().splitlines()
        return parent_spec + PathSpec.from_lines("gitwildmatch", lines)
    return parent_spec
```

**額外疊加**：
- `.dockerignore`（若存在）
- `.codebusignore`（CodeBus 專用，使用者可覆蓋）

**內建 always-ignore**（即使沒 .gitignore 也跳過）：
```
.git/  .hg/  .svn/
node_modules/  .venv/  venv/  __pycache__/  .mypy_cache/  .pytest_cache/
dist/  build/  target/  out/
.DS_Store  Thumbs.db
```

### 檔案大小上限
- `max_file_size_kb`（config，預設 512KB）
- 超過 → 分類為「超大檔」→ 只記 metadata，不讀內容（見六）

### 並行策略
- 遍歷單執行緒（I/O 快、邏輯單純不值得複雜化）
- 內容讀取 + sanitize：用 `asyncio.gather` 分批（batch size 20）
- Target：5000 檔 repo < 30s

---

## 四、類型判別（Binary / Text / 超大 / Generated）

> **Skeleton 範圍註記**：Binary 副檔名黑名單、null-byte 內容探測、lockfile / generated
> 副檔名與檔名 pattern、oversized 門檻均已實作。尚未實作：「非可印字符比例 > 30%」
> 的 binary 三段判別（目前靠 null-byte + encoding fallback 全失敗兩條判）、以及
> `oversized` 的 **檔頭 200 行 preview**（`FileEntry.oversized_preview` 目前永遠為 `None`）。

### Binary 偵測
**兩階段**：
1. **副檔名黑名單**：`.png .jpg .gif .ico .webp .mp3 .mp4 .wav .ogg .pdf .zip .tar .gz .bz2 .7z .exe .dll .so .dylib .class .pyc .o .a .lib .woff .woff2 .ttf .eot` → 直接判 binary，不讀
2. **內容探測**：讀前 8KB，用這條判斷是 binary：
   - 含 null byte（`\x00`）→ binary
   - UTF-8 decode 失敗 AND 其他編碼也失敗 → binary
   - 非可印字符比例 > 30% → binary

### Generated / Build Output 偵測
- 副檔名：`.min.js .min.css .bundle.js` → generated
- 檔名 pattern：`*-lock.json` / `yarn.lock` / `poetry.lock` / `Cargo.lock` / `uv.lock` / `Gemfile.lock` → lockfile
- 內容 heuristic：單行 > 5000 字元 → 很可能是 minified，歸 generated

**Lockfile / Generated 策略**（連動 README Module 1 需求）：
- **不讀內容**，不進 KB
- 但記 `path` 與 `size`，Explorer Agent 可看到「這個檔存在」但不會查內容
- 理由：內容對教材零價值，但存在本身有結構意義（告訴 Agent 這是個 npm 專案）

### 超大檔
- 大小 > `max_file_size_kb` 且非 binary
- 不讀內容，但記 metadata 與 **檔頭 200 行** summary（給 Explorer 預覽用）
- Tag 為 `oversized`，Explorer 可決定要不要 `read_file` partial range

---

## 五、編碼偵測

> **Skeleton 範圍註記**：fallback chain 全段已實作（UTF-8 / UTF-16 BOM / Big5 / GBK /
> Shift_JIS / `charset-normalizer` 保底），CJK codec 採用「lead-byte frequency 打分」
> 解 Big5/GBK/Shift_JIS byte-range overlap 的歧義。解碼全失敗 → `kind` 改判為 `binary`，
> `content` / `encoding` 皆 `None`。

嘗試順序（先成功者勝）：
1. **UTF-8**（含 BOM / 無 BOM）
2. **UTF-16 LE/BE**（有 BOM 才試，無 BOM 不猜）
3. **Big5**（CP950）
4. **GBK**（CP936）
5. **Shift_JIS**（CP932）
6. `charset-normalizer` library 猜測（保底）

所有失敗 → 判為 binary。

**不用 `chardet`**：`charset-normalizer` 較新、維護活躍、對中文偵測較準。但仍當 fallback，主要靠 fallback chain。

---

## 六、語言識別

> **Skeleton 範圍註記**：副檔名主路徑 + shebang 副路徑已實作；
> 未知副檔名且無 shebang → `language=None` / `language_confidence="unknown"`。
> 不使用 pygments / linguist。

### 策略
1. **副檔名**（主）：維護 `extension_to_lang` dict（`.py` → `python`、`.tsx` → `typescript` 等）
2. **Shebang**（副）：無副檔名時讀第一行 `#!/usr/bin/env python` 判斷
3. **不用 pygments / linguist**：依賴重、準度跟副檔名差不多

### 輸出欄位
```python
{
  "language": "python",
  "language_confidence": "extension" | "shebang" | "unknown"
}
```

---

## 七、Monorepo 子模組識別

### 偵測訊號（任一命中 → 標記為 monorepo）
- `pnpm-workspace.yaml` / `lerna.json` / `rush.json` / `nx.json`
- 根目錄 `package.json` 有 `workspaces` 欄位
- `Cargo.toml` 有 `[workspace]` section
- `go.work`
- `pyproject.toml` 有 `[tool.uv.workspace]` / `[tool.poetry.group]` 等

### 子模組清單
若為 monorepo，解析上述檔案取得 `packages/`、`apps/`、`libs/` 等子目錄清單，記入輸出：

```json
{
  "is_monorepo": true,
  "monorepo_type": "pnpm",
  "sub_packages": [
    { "path": "packages/core", "name": "@foo/core" },
    { "path": "apps/web", "name": "@foo/web" }
  ]
}
```

Module 4 Explorer 可據此決定「這個任務只需探索某個 sub-package」的限縮策略。

---

## 八、Git Metadata 收集（D-016 連動）

### 工具
**用 `pygit2`**（C binding，非 subprocess，符合 Tool Sandbox §十一）。

### 收集範圍（MVP）

| 項目 | 內容 | 為何要 |
|---|---|---|
| Repo meta | HEAD commit、current branch、remote url（sanitized）| Q&A 基本情境 |
| Recent commits | 最近 100 commits：oid / author / date / subject | 「最近改了什麼」類問題 |
| Per-file activity | 每檔：改動次數、最近改動時間、作者清單（top 3）| 「哪些檔最活躍」 |
| Blame（選擇性） | 只對 `top_N_files_by_activity`（預設 20）做完整 blame | 避免全 repo blame 爆 KB |

### Scope 與 Sanitize
- 只讀 `workspace_root/.git/`（Sandbox §十一）
- Commit message / author email / branch name / remote url 全過 Sanitizer（Scanner pass）
- 若 `.git/` 不存在 → 跳過本節，`git_metadata: null`

### 輸出格式
```json
{
  "git": {
    "head": "abc123...",
    "branch": "main",
    "remote_url": "<REDACTED:internal-domain>/org/repo.git",
    "recent_commits": [
      { "oid": "...", "author": "<REDACTED:email>", "date": "...", "subject": "..." }
    ],
    "file_activity": {
      "src/foo.py": { "commits": 42, "last_modified": "...", "authors": ["..."] }
    },
    "blame": {
      "src/foo.py": [
        { "line_start": 1, "line_end": 20, "oid": "...", "author": "..." }
      ]
    }
  }
}
```

---

## 九、Sanitize Orchestration（D-015 第一段）

### 流程
```
每個 text 檔解碼完 → ctx.sanitizer.sanitize(content, FileSource(pass_="scanner", path=rel_path))
                 → FileEntry.content = SanitizedResult.text（placeholder 版）
                 → FileEntry.sanitize_stats = 依 kind 聚合的 count dict
                 → SanitizerAuditLogger.append(...) 逐檔 flush
```

`scanner-sanitizer-orchestration` change 已把此流程串通；實作於
`sidecar/src/codebus_agent/scanner/service.py::_apply_pass1_sanitize`。

### 單次 session 全程記 `sanitize_audit.jsonl`（D-015 §三）

每個 Pass 1 命中寫一行，`source` 以結構化 `{"pass": "scanner", "path": <rel_path>}`
形式落盤（下游 Trust-Layer inspector 靠 `source.pass` filter）；`pass=1` /
`rules_version` / `session_id` 由 `/scan` endpoint 注入。

### 白名單觸發點
- Scanner 遍歷到檔案時先查 `sanitizer.local.yaml` 的 path / filename allowlist
- 命中白名單仍過 sanitize pattern，但記 `pass_through: true`（不替換，只記 audit）

### 失敗處理
- `ctx.sanitizer.sanitize` 拋任何例外 → 該檔 **不** 進 `ScanResult.files`、
  `warnings` 追加相對路徑、`ScanStats.quarantined_count += 1`；
  HTTP 仍 200（fail-closed 但不中止整體掃描）。

---

## 十、Sandbox 整合（D-017）

| 檢查點 | 做什麼 |
|---|---|
| Scanner 啟動 | 接收 workspace_root，呼叫 `resolve(strict=True)` 存入 ToolContext |
| 每個 entry（walk） | 先 `ensure_in_workspace(entry, ctx)`，不通過則 log + skip |
| Symlink resolve | `resolve(strict=False)` 檢查目標仍在 workspace |
| Git metadata 讀取 | `pygit2` 開啟 `workspace_root/.git/`，不走 subprocess |

**Scanner 不呼叫任何 tool registry 裡的 tool**——它是 tool 的 consumer 的上游，自己直接做。但所有 path 操作走同一組 sandbox helper。

---

## 十一、輸出資料格式

> **Skeleton 範圍註記**：schema 欄位**一次到位**，後續 change 不得破壞相容。目前
> 階段（skeleton + `scanner-sanitizer-orchestration`）的 stub 預設值：
>
> - `FileEntry.sanitize_stats` → 真實 kind→count（Pass 1 sanitize 後聚合），無命中時 `{}`
> - `FileEntry.oversized_preview` → 永遠 `None`（§四 檔頭 200 行 summary 未實作）
> - `FileEntry.content` → Pass 1 sanitized 字串（placeholder 形式 `<REDACTED:kind#N>`）；Pass 2 / Pass 3 由下游負責
> - `ScanResult.git` → 永遠 `None`
> - `ScanResult.is_monorepo=False` / `monorepo_type=None` / `sub_packages=[]`
> - `ScanStats.quarantined_count` → Pass 1 sanitize 失敗的檔案計數（成功通過時 `0`）
> - `ScanStats.total_files_included` = `len(files)`；`skipped_count` = `len(warnings)`

單次 scan 完成後產出 `ScanResult`：

```python
class FileEntry(BaseModel):
    path: str                       # 相對 workspace_root
    size: int
    kind: Literal["text", "binary", "oversized", "lockfile", "generated"]
    language: str | None
    language_confidence: str | None
    encoding: str | None            # text kind 才有
    content: str | None             # 已 sanitize；非 text kind 為 None
    oversized_preview: str | None   # oversized kind 才有
    sanitize_stats: dict[str, int]  # {"email": 2, "secret": 0, ...}

class Symlink(BaseModel):
    path: str
    target: str                     # 原 target 字串（sanitized）
    resolved_in_workspace: bool

class GitMeta(BaseModel):
    head: str
    branch: str
    remote_url: str | None          # sanitized
    recent_commits: list[dict]
    file_activity: dict[str, dict]
    blame: dict[str, list[dict]]

class ContentTypeSummary(BaseModel):
    """Repo 性質速覽，給 Explorer Agent 先決定策略用（不用再掃一次檔）"""
    total_files: int
    kind_counts: dict[str, int]          # {"text": 320, "binary": 12, "lockfile": 3, ...}
    language_counts: dict[str, int]      # {"python": 180, "typescript": 90, "markdown": 30, ...}
    category_counts: dict[str, int]      # {"code": 270, "docs": 35, "config": 15, "test": 40}
    dominant_category: Literal["code", "docs", "config", "mixed"]
    dominant_languages: list[str]        # top 3 by file count
    has_tests: bool                      # 偵測 tests/ __tests__/ *_test.py 等
    has_docs: bool                       # README / docs/ / *.md 聚集
    is_monorepo: bool                    # duplicate for quick access

class ScanStats(BaseModel):
    total_files_walked: int
    total_files_included: int
    total_bytes_read: int
    duration_seconds: float
    quarantined_count: int               # sanitize timeout / error 被隔離
    skipped_count: int                   # gitignore / size / binary skipped

class ScanResult(BaseModel):
    workspace_root: str
    scan_started_at: datetime
    scan_completed_at: datetime
    files: list[FileEntry]
    symlinks: list[Symlink]
    is_monorepo: bool
    monorepo_type: str | None
    sub_packages: list[dict]
    git: GitMeta | None
    content_summary: ContentTypeSummary  # ⭐ Explorer 先看這個決定策略
    stats: ScanStats
    warnings: list[str]                  # 解碼失敗、quarantined 等
```

這是 Module 2 KB Builder 的唯一輸入，也是 Module 4 Explorer 啟動時第一個讀的 context。

### Category 分類規則（衍生自 language + 路徑 heuristic）

| Category | 訊號 |
|---|---|
| `code` | language ∈ {python, typescript, javascript, rust, go, java, c, cpp, ...} 且路徑非 tests/docs |
| `docs` | language ∈ {markdown, rst, asciidoc} 或 路徑含 `docs/` / `README*` / `CHANGELOG*` |
| `config` | language ∈ {yaml, toml, json, ini} 或 檔名 `*.config.*` / `Dockerfile` / `Makefile` |
| `test` | 路徑含 `tests/` / `test/` / `__tests__/` / `spec/`，或檔名 `*_test.py` / `*.test.ts` / `*.spec.ts` |
| `other` | 以上皆非 |

`dominant_category` 判定：
- 若最大類佔 > 60% → 該類（`code` / `docs` / `config`）
- 否則 → `mixed`

### 為何放這層（而非 Explorer 自己算）
- Scanner 已走過所有檔，順手統計零成本
- Explorer 啟動若沒這 summary 就要先 `list_dir` 探 repo 全貌才知道策略，浪費 budget
- 支援 README §四 MVP 項「自動判斷內容類型（程式碼、技術文件、混合）」

---

## 十二、失敗處理

| 情況 | 處理 |
|---|---|
| 檔案讀取 permission denied | log warning，skip，繼續 |
| Decode 失敗（所有 encoding 都 fail） | 改判為 binary，不中止 |
| Sanitize 正則 timeout | 檔案進 quarantine，不進 KB |
| Git metadata 讀取失敗（repo corrupt） | git 部分為 null，其他正常繼續 |
| workspace_root 不存在 / 不可讀 | 直接 fail，回 sidecar error code `SCANNER_WORKSPACE_INVALID` |
| 單次 scan 記憶體爆 | 無硬上限，但 `max_total_bytes`（預設 500MB）超過觸發 warning，建議使用者縮範圍 |

---

## 十三、效能與進度回報

> **Skeleton 範圍更新（2026-04-22，change `sse-progress-skeleton`）**：
> `POST /scan` 仍**預設同步**單 body JSON response，但新增 `?stream=true`
> opt-in async streaming 路徑——建立 task → spawn background coroutine 跑
> `scan(..., on_progress=…)` → 立即回 `{"task_id": "scan_<hex8>"}`；訂閱者
> 透過 `GET /tasks/{id}/events` 收 progress / done / error。效能 target
> 仍未在骨架做 benchmark 驗證。

### Target
| Repo 規模 | 目標時間 |
|---|---|
| < 500 檔 | < 5s |
| 500-2000 檔 | < 15s |
| 2000-5000 檔 | < 30s |
| > 5000 檔 | 無硬保證，每 500 檔 emit progress |

### 進度回報介面（landed）

對應 `openspec/changes/sse-progress-skeleton/specs/folder-scanner/spec.md`
Requirements `Scanner progress callback hook` 與 `POST /scan opt-in async
streaming mode`。

**`ScannerProgressCallback` Protocol**（`scanner/models.py`）：

```python
ScannerProgressCallback = Callable[[ScannerProgressEvent], Awaitable[None]]

class ScannerProgressEvent(BaseModel):
    phase: Literal["walking", "sanitizing"]
    current: int            # 已處理檔案數
    total: int | None       # walking 階段未知；sanitizing 階段為 walking 結果
    current_file: str | None
```

`scanner.service.scan(...)` 是 async function，接受 `on_progress:
ScannerProgressCallback | None = None`。`_PROGRESS_EMIT_EVERY = 50`
控制兩階段的 emit 節奏（每 50 檔一次，加上每階段尾端的 guarantee event，
確保小 workspace 也至少各 emit 一次 `walking` / `sanitizing`）。

### Wire 翻譯與 SSE 事件

`api/scan.py::_scanner_event_to_wire(event)` 把兩個 source phase 折疊成
單一 wire phase `"scanning"`（消費端不需感知 scanner 內部 pipeline 切分）：

```json
{ "type": "progress", "phase": "scanning", "current": 420, "total": 940, "current_file": "src/foo.py" }
```

- 對外只看到 `phase: "scanning"`，內部 `walking` / `sanitizing` 兩階段透明
- `total` 在 walking 階段為 `null`，sanitizing 階段則為 walking 完成後的總數
- 每 50 檔 emit 一次（避免 SSE 洪水）；終端的 SSE `done` 事件由 task wrapper
  發出，`scan()` 自身不感知 transport 層（詳見 `sidecar-api.md §三-bis`）

---

## 十四、測試與 fixture

### 單元測試
- gitignore 繼承（多層巢狀）
- Binary 偵測（各種 edge case：有 BOM 的 UTF-16、含大量非 ASCII 的中文 log）
- Encoding fallback chain
- Symlink 處理（指向 workspace 內 / 外 / 循環）

### Integration fixture
`tests/fixtures/scanner/` 準備：
- `mini-ts-repo/`：5 檔 TypeScript，有 .gitignore、node_modules 要跳過
- `mini-py-repo/`：10 檔 Python，有 `__pycache__`、`.venv`
- `monorepo-pnpm/`：根 package.json workspace + packages/{a,b}
- `mixed-encoding/`：混 utf-8 + big5 + binary
- `with-secrets/`：檔案含 fake API key，驗 sanitize orchestration

### E2E
對 Demo repo（Timeline，D-004）跑一次 scan，比對人工盤點的檔案清單（哪些該被 sanitize、monorepo 判定等）。

---

## 十五、MVP 不做

| 項 | 延後原因 |
|---|---|
| 增量 scan（只掃改動過的檔）| MVP 每次重掃 |
| File watch / hot reload | Phase 2 |
| `git log --follow` 處理檔案重命名 | 需求未證實 |
| 外部 include（symlink 指 workspace 外的允許白名單）| 合規複雜 |
| 二進制檔內容 metadata（PDF / docx 文字抽取）| 範圍擴大 |
| Binary 內容 fuzzy hash 去重 | Phase 2 做 KB 去重時再考慮 |
| 多 workspace 同時掃 | Sidecar 當下一次一個 task（`sidecar-api.md` §七）|

---

## 十六、實作順序

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | 檔案遍歷 + `.gitignore` + 內建 always-ignore | 0.5d |
| P0 | Binary / encoding 偵測 | 0.5d |
| P0 | Sandbox helper 整合 | 0.25d |
| P0 | Sanitizer orchestration 串接 | 0.25d |
| P0 | Lockfile / generated / oversized 分類 | 0.25d |
| P0 | 輸出 ScanResult + SSE progress | 0.25d |
| P0 | Content-type summary 統計與 category 分類 | 0.25d |
| P1 | Monorepo 偵測與子模組解析 | 0.5d |
| P1 | Git metadata 基礎（pygit2 + recent commits + file activity） | 1d |
| P1 | Per-file blame（top-N 檔案） | 0.5d |
| P1 | 測試 fixture + E2E 對 Timeline | 0.5d |

**合計 P0 ~2.25d / P0+P1 ~4.75d。**
