# Timeline Storage Adapter — Synthetic Golden Fixture

> ⚠️ **這是合成 fixture，不是真 Timeline repo 的鏡像。** 9 個檔案是手寫的最小 stub，用來驗證 Explorer Agent 在「新加 Storage Adapter」這類任務上產出的路線是否命中該命中的檔案。

## 對應的人工分析

本 fixture 對齊 [`tests/golden/timeline-gdrive-adapter/ideal-route.md`](../timeline-gdrive-adapter/ideal-route.md) 的 5 站理想路線（站 1: 介面 → 站 2: Mock → 站 3: LocalFile → 站 4: 掛載 → 站 5: Quiz）。檔名對齊真 Timeline 的 `app/...` 結構，但內容是合成 stub（≤ 40 行/檔，禁編譯）。

## 結構

```
timeline-storage-adapter-synthetic/
├── README.md                     ← 本檔
├── ideal-route.json              ← IdealRoute Pydantic 機器讀版本（9 檔分類）
└── workspace/
    ├── README.md                 ← noise（純 repo readme）
    └── app/
        ├── types/index.ts                    ← must_have（站 1：介面）
        ├── services/MockStorageAdapter.ts    ← must_have（站 2：Mock）
        ├── services/LocalFileAdapter.ts      ← must_have（站 3：LocalFile）
        ├── composables/useStorage.ts         ← must_have（站 4：掛載）
        ├── stores/timeline.ts                ← must_have（站 4：consumer）
        ├── stores/node.ts                    ← nice_to_have（次要 consumer）
        ├── stores/settings.ts                ← nice_to_have（次要 consumer）
        └── components/EventCard.vue          ← noise（UI off-route）
```

## ideal-route.json schema

對齊 `sidecar/tests/golden/scoring.py::IdealRoute`：四欄 `task` / `must_have` / `nice_to_have` / `noise_paths`。路徑用相對 `workspace/...` 形式。

## 用途

1. **目前**：`sidecar/tests/golden/test_timeline_synthetic_replay.py` 用 scripted MockProvider 跑 5 step replay，assertion 命中 must_have 全綠且無 noise。
2. **未來打磨期**（D-006 `[ ] 真 LLM snapshot` 待 follow-up change）：把 MockProvider 換 OpenAIChatProvider 跑真 LLM、snapshot baseline。fixture 結構與 scoring helper 都已 LLM-agnostic，介面零改動。
