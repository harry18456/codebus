# Scanner fixtures

Backs `openspec/changes/scanner-skeleton/tasks.md` Task 8.1.

每個子資料夾都是一個可被 `scan(workspace_root, ctx)` 吃的迷你 workspace，
用來驗 scanner 的 end-to-end 行為（不取代各 leaf module 的單元測試）：

| Fixture | 目的 |
| --- | --- |
| `mini-py-repo/` | Python workspace：包含 `__pycache__/` 驗 built-in ignore，`tests/` 下有 `*_test.py` 驗 `has_tests`。 |
| `mini-ts-repo/` | TypeScript workspace：根 `.gitignore` 列 `node_modules/`，實際也放 `node_modules/foo/index.js` 驗 built-in + gitignore 兩條規則都生效。 |
| `mixed-encoding/` | 三種檔：UTF-8（README）、Big5 bytes、null-byte binary，驗 encoding fallback chain + NUL sniff。 |
| `symlink-cases/` | POSIX-only：包含 in-workspace / out-of-workspace 兩支 symlink。Windows 上由 test runner 自動 skip。 |

**注意**：fixtures 內部不塞 `.git/` 目錄，避免污染主 repo。測試需要觸發 `.git/`
built-in ignore 行為時，改用 pytest 的 `tmp_path` 在測試 runtime 建立。
