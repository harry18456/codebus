# Tool Sandbox Spec — Agent 工具執行邊界

> 管 **Agent 能摸哪些檔案、走哪些命令、寫哪些位置**。
> **這不是 Sanitizer**（Sanitizer 管「送 LLM 的字清不清」；Sandbox 管「Agent 可以動到什麼」）。
> 關聯決策：**D-017**、D-011（資安）、D-015（Sanitizer）、D-016（Q&A）。
> 關聯文件：`security.md`（合規映射）、`agent-core.md` §六（ToolContext）、`sanitizer.md`（資料外流防線）。

---

## 一、兩層防線的再確認

```
┌─────────────────────────────────────────────────┐
│ 使用者授權的 workspace_root                     │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │ Tool Sandbox（本 spec）                  │   │
│  │ ↑ Agent 只能在框內讀 / 不能 exec / 只能  │   │
│  │   寫 KB（不碰 filesystem）               │   │
│  └──────────────────────────────────────────┘   │
│                     │                           │
│                     ▼ 拿到內容                  │
│  ┌──────────────────────────────────────────┐   │
│  │ Sanitizer（D-015）                       │   │
│  │ ↑ 去識別化後才送 LLM                     │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
└─────────────────────────────────────────────────┘
```

- 進站（Agent 拿資料）= Sandbox 管
- 出站（送出 LLM）= Sanitizer 管
- **兩層獨立、各自可測、都不信任對方**

---

## 二、管制對象

| 對象 | MVP 政策 |
|---|---|
| **檔案讀取**（`read_file` / `list_dir` / `search` / `trace_import` / `find_callers`） | 限 `workspace_root` 子樹 |
| **檔案寫入**（code / data） | **完全禁止**——Agent 沒有寫本地檔案的工具 |
| **KB 寫入**（`add_to_kb`，D-016） | 只寫 Qdrant 本地 collection，不透過 filesystem path |
| **Shell / exec / subprocess** | **完全禁止**（MVP read-only 承諾） |
| **網路存取** | 只准走 Provider 層的 LLM API endpoint；任何 tool 不得直接 http fetch |
| **環境變數讀取** | 工具可讀，但**結果進 Sanitizer 必過**（避免無意洩漏） |
| **process 資訊 / 系統資訊** | 不開放（沒對應 tool） |

**白名單即 tool registry**（`agent-core.md` §六）——沒註冊的 tool 完全無法被呼叫，LLM 若產出不明 tool name 直接丟 error 給它。

---

## 三、路徑白名單

### workspace_root 定義

使用者在首次授權 modal（`sanitizer.md` §七）選定的資料夾，經 `Path.resolve(strict=True)` 處理後存入 `ToolContext.workspace_root`。整個 sidecar session 期間不可變。

### 額外允許位置

| 位置 | 用途 | 讀 | 寫 |
|---|---|---|---|
| `workspace_root/**` | 使用者 codebase | ✅ | ❌ |
| `~/.codebus/sanitizer.local.yaml` 等 config | Sanitizer / App 設定 | ✅（App 層） | ❌（tool 層不可） |
| `{codebus_workspace}/kb/` | Qdrant 本地 collection | 僅透過 Qdrant client | 僅透過 Qdrant client |
| `{codebus_workspace}/reasoning_log.jsonl` 等 log | Agent 稽核 | 僅透過 Logger | 僅透過 Logger |
| `workspace_root/.git/` | git metadata（D-016 連動） | ✅（只讀） | ❌ |

**其他全部拒絕**：`/etc/`、`~/.ssh/`、`/tmp/`、`/proc/`、`C:\Windows\`、其他使用者 home 等。

### 路徑驗證 helper（Python）

```python
# sidecar/src/codebus_agent/agent/sandbox.py
from pathlib import Path

class PathEscapeError(ValueError):
    """請求的路徑逃出 workspace"""

def ensure_in_workspace(requested: str | Path, ctx: ToolContext) -> Path:
    """
    標準化 + resolve + 檢查在 workspace 底下。
    任何 tool 要接 path 參數，**必須**先走這個 helper。
    """
    # 1. 轉絕對路徑 + resolve symlink
    try:
        p = (ctx.workspace_root / requested).resolve(strict=True)
    except FileNotFoundError:
        # 也允許不存在的路徑（但必須在 workspace）— 用 strict=False 再一次
        p = (ctx.workspace_root / requested).resolve(strict=False)
        if not _is_relative_to(p, ctx.workspace_root):
            raise PathEscapeError(f"Path {requested!r} not under workspace")
        return p

    # 2. 驗證在 workspace 底下（Path.is_relative_to 需 3.9+）
    if not _is_relative_to(p, ctx.workspace_root):
        raise PathEscapeError(
            f"Resolved path {p} escapes workspace {ctx.workspace_root}"
        )
    return p


def _is_relative_to(p: Path, root: Path) -> bool:
    """Path.is_relative_to 的相容版（處理 Windows 大小寫）"""
    try:
        p.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False
```

### 檢查重點

1. **先 resolve 再比對**：防止 `../../etc/passwd` 或 `workspace/../../` 逃脫
2. **處理 symlink**：`resolve(strict=True)` 會跟隨 symlink 到真實位置再比——若 symlink 指向 workspace 外，`is_relative_to` 會 fail
3. **Unicode normalization**：Path 內部處理，但 search tool 拿 query 字串要 `unicodedata.normalize("NFC", s)` 避免同字 different encoding 繞過
4. **大小寫差異**（macOS HFS+ / Windows NTFS case-insensitive）：不額外處理，`resolve()` 會正規化到實體名稱
5. **NTFS ADS**（`file.txt:stream`）：不特別過濾——Python `Path` 會 resolve 到主檔，讀的內容是 stream 則由 OS 決定

---

## 四、Tauri 端 fs.scope

`tauri/tauri.conf.json`：

```json
{
  "plugins": {
    "fs": {
      "scope": {
        "allow": [
          "$HOME/.codebus/**",
          "$APPDATA/codebus/**"
        ],
        "deny": [
          "$HOME/.ssh/**",
          "$HOME/.aws/**",
          "$HOME/.config/**"
        ]
      }
    }
  }
}
```

**workspace_root 不寫死在 conf**——使用者選定後透過 Tauri `fs::add_scope` runtime 加入，關閉 App 時解除。這樣允許不同專案 session。

前端（Nuxt）只能透過已授權的 scope 讀檔，直接打本機路徑會被 Tauri runtime 擋。

---

## 五、Python Sidecar 執行期

### ToolContext（agent-core.md §六 擴充）

```python
class ToolContext(BaseModel):
    workspace_root: Path              # 已 resolve，唯一真相
    workspace_id: str
    session_id: str                   # workspace open → close 完整生命週期（非 per-agent-run；scan/kb_build/explore/generate/qa 全共用，D-021）
    kb: KnowledgeBase                 # Qdrant client wrapper
    kb_growth_log: KBGrowthLogger     # D-016
    sanitizer: Sanitizer              # D-015
    audit_log: AuditLogger            # 本 spec §七
    usage_tracker: UsageTracker       # D-021（agent-core.md §十三）
    allow_git_metadata: bool = True   # 是否允許讀 .git/

    class Config:
        frozen = True  # 建立後不可變
```

### 每個工具的責任

所有接 path 參數的 tool 必須：
```python
@tool(name="read_file", schema=ReadFileArgs, description="...")
async def read_file(args: ReadFileArgs, ctx: ToolContext) -> str:
    p = ensure_in_workspace(args.path, ctx)  # ← 這行必做
    # ... 繼續讀
```

漏寫就是 bug。Lint / review 規則列為必查項。

### Tool 執行器統一保護

`agent-core.md` §六 的 `_execute_one` 再加一層 try：

```python
async def _execute_one(call, tools, ctx) -> ToolResult:
    try:
        tool = tools.get(call.name)
        if tool is None:
            return ToolResult(error=f"Unknown tool: {call.name}", ...)
        args = tool.schema.model_validate(call.arguments)
        output = await tool.fn(args, ctx)
        return ToolResult(output=output, ...)
    except PathEscapeError as e:
        ctx.audit_log.write("path_escape_attempt", call=call, error=str(e))
        return ToolResult(error=f"PATH_ESCAPE: {e}", ...)
    except Exception as e:
        return ToolResult(error=f"ERROR: {e}", ...)
```

---

## 六、重複違規處理

**單次**：tool 回 error，Agent 在下輪收到 observation「PATH_ESCAPE: ...」會自己換招

**連續同 session 超過 5 次 path escape 嘗試**：
1. 寫 audit 警告 `"state": "suspicious"`
2. Explorer / Q&A 迴圈提早收斂
3. UI 顯示警告「Agent 多次嘗試離開 workspace，已停止」
4. 提供使用者「查看嘗試紀錄」按鈕

MVP 不做的：自動 terminate sidecar、封鎖 provider。過度反應可能誤殺 prompt injection 以外的情境（Agent 只是 confused）。

---

## 七、稽核

### `tool_audit.jsonl`（每次 tool 呼叫寫一筆）

```json
{
  "ts": "2026-04-17T10:30:00Z",
  "session_id": "...",
  "step": 7,
  "tool": "read_file",
  "args": { "path": "src/foo.py", "line_range": "1-100" },
  "status": "ok",
  "bytes_returned": 2431,
  "duration_ms": 12
}
```

拒絕的嘗試：
```json
{
  "ts": "...",
  "tool": "read_file",
  "args": { "path": "../../../etc/passwd" },
  "status": "path_escape",
  "resolved_to": "/etc/passwd",
  "workspace_root": "~/projects/timeline"
}
```

**不記檔案內容**（那是 reasoning_log 的事）——這份只記 who/what/status/size。

UI 「稽核報告」tab 下有三分頁：
- 🛡️ Sanitizer（替換統計）
- 🔒 Tool Sandbox（本 spec 的 audit）
- 📚 KB Growth（D-016）

---

## 八、測試（Red Team fixture）

`tests/sandbox/attacks/` 準備一組 Agent 可能嘗試的 escape path，每個都應被擋：

```
../../etc/passwd
/etc/passwd
~/.ssh/id_rsa
{workspace}/../sibling_repo/secret
{workspace}/symlink_to_etc/passwd
{workspace}/..%2Fetc%2Fpasswd
C:\Windows\System32\config\SAM
\\?\C:\Users\other\.ssh\id_rsa
file:///etc/passwd
```

CI 跑這組，每個 `ensure_in_workspace` 都應 raise `PathEscapeError`。

---

## 九、與 Explorer / Q&A / Scanner 的對接

| 模組 | 使用 Sandbox 的方式 |
|---|---|
| **Module 1 Scanner** | 起點是 workspace_root；遞迴掃時每個 entry 都過 `ensure_in_workspace`（symlink 防護） |
| **Module 4 Explorer Agent** | 每個 tool 實作內呼 helper；tool registry 本身就是白名單 |
| **Module 8 Q&A Agent** | reuse Explorer tools，加上 `add_to_kb` 不透過 filesystem path，天然在 sandbox 外 |
| **Git metadata 收集**（D-016 連動） | 只讀 `workspace_root/.git/`，透過 `git` Python binding 或 `pygit2`，**不走 subprocess**（避免 exec surface） |

---

## 十、Network 邊界

### 允許
- Provider 層（`llm-provider.md`）對 LLM API 的 call
- Qdrant client 對 localhost Qdrant 的 call

### 禁止
- 任何 tool 內部直接 `httpx` / `requests` / `urllib` 打外部
- DNS lookup（除上述兩者所需）
- WebSocket / gRPC 對外

**強化手段（建議加但 MVP 可選）**：sidecar 啟動時用 monkey-patch 或 `socket.socket` hook 攔截非白名單連線。MVP 先靠 code review，Phase 2 加 runtime 防護。

---

## 十一、Git metadata 存取

D-016 Q&A Agent 需要 git history。Sandbox 策略：

1. **讀取方式**：用 `pygit2` library（C binding，不 spawn subprocess）
2. **範圍**：只讀 `workspace_root/.git/`
3. **可用資訊**：
   - `git log` → commit list（oid / author / date / message）
   - `git blame` → 每行 author / commit
   - `git show <commit>` → diff 內容
4. **Sanitize**：commit author email、commit message 內容都過 Sanitizer（D-015）
5. **禁用**：`git push` / `git commit` / `git checkout` / 任何寫操作——`pygit2` 能做但 tool 不暴露

若未來需要 subprocess `git`（pygit2 涵蓋不到），再寫 exec allowlist spec，屆時進 `docs/exec-allowlist.md`。

---

## 十二、合規對映（連動 security.md）

| 合規要求 | Sandbox 對應機制 |
|---|---|
| 「Agent 不破壞使用者 code」 | 沒任何寫 filesystem 的 tool |
| 「不開放服務埠對外」 | Sidecar bind 127.0.0.1（`sidecar-api.md` §一）+ Network 邊界本 spec §十 |
| 「agentic 行為可稽核」 | `tool_audit.jsonl` §七 |
| 「不讀取使用者未授權位置」 | workspace_root 白名單 + path resolve 檢查 |
| 「遠端熔斷機制」 | Cancel 走 `asyncio.Event`（agent-core §四） |

---

## 十三、失敗處理

| 情況 | 處理 |
|---|---|
| `resolve(strict=True)` 遇不存在路徑 | 改用 `strict=False` + 驗 parent 在 workspace 內 |
| Symlink 指向 workspace 外 | `resolve` 後 `is_relative_to` 失敗 → `PathEscapeError` |
| Symlink loop | `resolve` 會 raise `RuntimeError` → 視為 path error |
| workspace_root 本身是 symlink | 啟動時先 `resolve` 一次取實體路徑存入 ctx |
| 路徑含 null byte | Python `Path` 接受但 OS 層會拒；tool 層在 helper 加檢查 `"\x00" in str(p)` |
| 極長路徑（Windows > 260） | 靠 OS 層 error，不特別處理 |

---

## 十四、MVP 不做

| 項 | 延後原因 |
|---|---|
| Linux seccomp / capabilities 限制 | OS 層 sandbox 複雜；Python 層限制 MVP 夠 |
| Docker / namespace 隔離 | 打包複雜度爆炸 |
| eBPF 稽核 | 需 root，不適合桌面 App |
| Windows AppContainer | Tauri 生態支援未成熟 |
| 網路層 runtime hook（socket patch） | Phase 2，MVP 靠 code review |
| 動態權限升降（Agent 臨時要更高權限再要求授權） | 需要更多 UX 設計 |

---

## 十五、實作順序

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | `ensure_in_workspace` helper + test fixture | 0.5d |
| P0 | ToolContext frozen + workspace_root resolve 初始化 | 0.25d |
| P0 | 所有既有 tool 套用 helper | 0.5d |
| P0 | `tool_audit.jsonl` logger | 0.25d |
| P0 | Tauri fs.scope runtime 加入 workspace_root | 0.5d |
| P0 | Red team 測試 fixture + CI | 0.5d |
| P1 | 重複違規熔斷（5 次 escape → 停止） | 0.25d |
| P1 | 稽核 UI（Tool Sandbox tab） | 1d |
| P1 | `pygit2` 整合 + git metadata read | 1d |
| P2 | Network runtime hook | 後議 |

**合計 P0 ~2.5d / P0+P1 ~4.75d。**
