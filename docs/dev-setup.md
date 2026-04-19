# Dev Setup — 開發環境與 CodeBus 啟動指南

> 從 clone 到跑起來的 onboarding 指南。
> 關聯決策：D-001（混合架構）、D-013（monorepo）、D-014（uv toolchain）、D-026（web 用 npm）。

---

## 一、先決條件

| 工具 | 版本 | 用途 |
|---|---|---|
| Rust | >= 1.80（stable） | Tauri 殼編譯 |
| Node | >= 20 | Nuxt3 前端 + 內建 npm（D-026） |
| Python | >= 3.11 | Sidecar runtime（uv 會處理） |
| uv | latest | Python 套件 + venv（D-014） |
| Docker（或 Qdrant binary） | — | 本地 Qdrant（dev 便利） |
| Git | >= 2.40 | 基本 |

### 安裝指令（Linux / macOS）

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node（含 npm）— 用 nvm 範例；macOS / Linux 也可用 brew / package manager
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
nvm install 20 && nvm use 20

# uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# Qdrant（Docker 版，最簡單）
docker run -p 6333:6333 -p 6334:6334 -v $(pwd)/qdrant_storage:/qdrant/storage qdrant/qdrant
```

Windows：Rust / Node / uv / Docker 皆有 Windows installer，參官方。

---

## 二、Repo 初始化

```bash
git clone <repo-url> codebus
cd codebus
```

目錄結構（D-013）：
```
codebus/
├── tauri/          # Rust 殼
├── sidecar/        # Python agent + API
├── web/            # Nuxt3 前端
├── docs/
├── tests/
│   ├── golden/
│   ├── fixtures/
│   └── sandbox/
└── README.md
```

---

## 三、Python Sidecar 設定

```bash
cd sidecar

# 首次：uv 自動建 venv + install
uv sync

# 跑 lint / test
uv run pytest
uv run ruff check .
uv run pyright

# 單獨啟 sidecar（dev 模式）
uv run python -m codebus_agent.api --dev
# → 預設 bind 127.0.0.1:<random-port>，token 印 stdout
```

### 環境變數

建 `sidecar/.env`（已加進 .gitignore）：

```env
# LLM 供應商 API
CODEBUS_LLM_PROVIDER=contest
CODEBUS_LLM_API_BASE=https://...        # TODO review: 實際端點
CODEBUS_LLM_API_KEY=sk-...              # 不要 commit
CODEBUS_CHAT_MODEL=...                  # TODO review
CODEBUS_EMBED_MODEL=...                 # TODO review

# Qdrant
CODEBUS_QDRANT_URL=http://127.0.0.1:6333

# Workspace（CodeBus 專案 metadata 存放）
CODEBUS_WORKSPACE=~/.codebus
```

---

## 四、前端 (Nuxt3)

```bash
cd web

npm install          # 首次
npm run dev          # http://localhost:3000
npm run typecheck
npm run lint
```

前端開發期間可脫離 Tauri 殼直接跑 `npm run dev`，sidecar URL 用 `.env.local` 指到 dev sidecar。

---

## 五、Tauri 殼

```bash
cd tauri

cargo check          # 首次
cargo tauri dev      # 起整個 app（自動 spawn sidecar + web）
```

`tauri dev` 會：
1. 啟動 web dev server（透過 `beforeDevCommand`）
2. spawn Python sidecar（透過 `externalBin` 設定）
3. 開 WebView 視窗

### `tauri.conf.json` 關鍵

- `build.beforeDevCommand`：`cd ../web && npm run dev`
- `build.frontendDist`：`../web/dist`
- `bundle.externalBin`：`../sidecar/dist/codebus_agent`（PyInstaller 產物）
- `plugins.fs.scope`：見 `tool-sandbox.md` §四

---

## 六、Qdrant 本地啟動

```bash
# Docker（推薦 dev）
docker run -d \
  --name codebus-qdrant \
  -p 6333:6333 -p 6334:6334 \
  -v $HOME/.codebus/qdrant_storage:/qdrant/storage \
  qdrant/qdrant

# 或直接 binary（Linux）
wget https://github.com/qdrant/qdrant/releases/download/<ver>/qdrant-x86_64-unknown-linux-gnu.tar.gz
tar xf qdrant-*.tar.gz
./qdrant
```

檢查：`curl http://127.0.0.1:6333/healthz`。

---

## 七、Sanitizer Config

首次啟動前建 `~/.codebus/sanitizer.local.yaml`：

```yaml
# 公司 / 環境特定清單，不進 git
internal_domains: []
internal_hostname_patterns: []
extra_secret_patterns: []

path_allowlist: []
filename_allowlist:
  - ".env.example"
  - ".env.sample"

options:
  enable_entropy_suspect: false
  max_file_size_kb: 512
  regex_timeout_ms: 5000
```

詳見 `docs/sanitizer.md` §五。

---

## 八、完整 Smoke Test（首次跑起來）

```bash
# Terminal 1: Qdrant
docker start codebus-qdrant

# Terminal 2: sidecar（背景）
cd sidecar && uv run python -m codebus_agent.api --dev

# Terminal 3: Tauri（會自動起 web + 連 sidecar）
cd tauri && cargo tauri dev

# 在 App 內：
# 1. 選 workspace 資料夾（建議用 tests/fixtures/scanner/mini-py-repo）
# 2. 同意授權 modal
# 3. 等 scan → embedding → explore → generate
# 4. 應看到 Agent console、生成的 tutorial、站牌列表
```

---

## 九、測試

### Python
```bash
cd sidecar
uv run pytest                      # 全部
uv run pytest tests/unit/          # 單元
uv run pytest tests/integration/   # 整合
uv run pytest tests/golden/        # golden sample regression
```

### Rust
```bash
cd tauri
cargo test
```

### 前端
```bash
cd web
npm test
```

### Red Team（Sandbox）
```bash
cd sidecar
uv run pytest tests/sandbox/        # path escape 等攻擊 fixture
```

---

## 十、打包

```bash
# 1. 先打 Python sidecar 成 binary
cd sidecar
uv run pyinstaller --onefile \
  --name codebus_agent \
  --distpath ./dist \
  src/codebus_agent/api/__main__.py

# 2. 再打 Tauri App（會自動抓 externalBin）
cd tauri
cargo tauri build
# → 產出 AppImage / MSI / dmg 在 target/release/bundle/
```

### 驗證打包
- 在「乾淨」測試機（無 Python / Rust）跑安裝後版本
- 確認 sidecar binary 內嵌、啟動時無需外部依賴

---

## 十一、常見問題

### `uv sync` 卡住
- 確認網路能連 PyPI
- `uv sync --refresh` 強制重抓

### Tauri 找不到 sidecar
- 確認 `sidecar/dist/codebus_agent` 存在（Linux）/ `.exe`（Windows）
- 檢查 `tauri.conf.json` `bundle.externalBin` 路徑

### Qdrant 連不上
- `docker ps` 確認 container 跑起來
- 防火牆沒擋 6333

### 授權 modal 卡住
- 檢查 `~/.codebus/` 有寫入權限
- 看 sidecar stdout 是否有 error

### Pre-commit hook 失敗
- Python：`uv run ruff check --fix .` + `uv run pyright`
- Rust：`cargo fmt && cargo clippy`
- Frontend：`npm run lint -- --fix`

---

## 十二、CI

（初版草案，實作後補入 `.github/workflows/`）

| Job | 做什麼 |
|---|---|
| python-check | `uv sync --frozen` + ruff + pyright + pytest |
| rust-check | `cargo fmt --check` + clippy + test |
| web-check | `npm ci` + typecheck + lint + test |
| golden-regression | 對 Timeline + GDrive fixture 跑 Explorer + Generator，比對 baseline |
| red-team | `tests/sandbox/` 全跑 |
| build-smoke | `cargo tauri build` + 驗 binary 可啟動 |

---

## 十三、待 review 的決策

- LLM 供應商實際 endpoint / model name（`.env` 範本有 `TODO review` 註記）
- CI provider（GitHub Actions vs 自建）
- 打包簽章（Windows SmartScreen / macOS notarization 是否 Phase 1 處理）
