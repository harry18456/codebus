import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useWikiStore } from "./wiki"

const invokeMock = vi.mocked(invoke)
const listenMock = vi.mocked(listen)

describe("useWikiStore", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  it("useWikiStore_listPages_populates_pages_index", async () => {
    invokeMock.mockResolvedValueOnce([
      { slug: "uv-lib", path: "/v/.codebus/wiki/modules/uv-lib.md", title: "uv-lib" },
      { slug: "uv-child", path: "/v/.codebus/wiki/modules/uv-child.md", title: "uv-child" },
      { slug: "cache", path: "/v/.codebus/wiki/concepts/cache.md", title: "Cache" },
    ])
    await useWikiStore.getState().listPages("/v")
    const pages = useWikiStore.getState().pages
    expect(Object.keys(pages)).toHaveLength(3)
    expect(pages["uv-lib"].title).toBe("uv-lib")
    expect(pages["cache"].title).toBe("Cache")
  })

  it("useWikiStore_loadPage_caches_body", async () => {
    invokeMock.mockResolvedValueOnce("# uv-lib\nbody")
    await useWikiStore.getState().loadPage("/v", "uv-lib")
    expect(useWikiStore.getState().body).toContain("body")
    expect(useWikiStore.getState().currentPath).toBe("uv-lib")

    // Second load with the same slug must hit the cache and SHALL
    // NOT issue another IPC call.
    await useWikiStore.getState().loadPage("/v", "uv-lib")
    expect(invokeMock).toHaveBeenCalledTimes(1)
  })

  it("loadPage updates currentPath and body for a fresh slug", async () => {
    invokeMock.mockResolvedValueOnce("first body")
    await useWikiStore.getState().loadPage("/v", "a")
    expect(useWikiStore.getState().currentPath).toBe("a")
    expect(useWikiStore.getState().body).toBe("first body")

    invokeMock.mockResolvedValueOnce("second body")
    await useWikiStore.getState().loadPage("/v", "b")
    expect(useWikiStore.getState().currentPath).toBe("b")
    expect(useWikiStore.getState().body).toBe("second body")
  })

  it("reset clears all in-memory state", async () => {
    invokeMock.mockResolvedValueOnce("x")
    await useWikiStore.getState().loadPage("/v", "page")
    useWikiStore.getState().reset()
    const state = useWikiStore.getState()
    expect(state.pages).toEqual({})
    expect(state.currentPath).toBeNull()
    expect(state.body).toBeNull()
    expect(state._bodyCache).toEqual({})
  })

  it("wiki store subscribes to goal-terminal channel at init", () => {
    // The store factory ran at module import; verify the subscription
    // was registered.
    const channels = listenMock.mock.calls.map(([channel]) => channel)
    expect(channels).toContain("goal-terminal")
  })

  it("useWikiStore_onTerminal_relists_pages_after_goal_completes", async () => {
    // First listPages call: agent run hasn't started, wiki/ has 1 page.
    invokeMock.mockResolvedValueOnce([
      { slug: "a", path: "/v/.codebus/wiki/modules/a.md", title: "A" },
    ])
    await useWikiStore.getState().listPages("/v")
    expect(Object.keys(useWikiStore.getState().pages)).toEqual(["a"])

    // Goal completes — terminal handler re-runs listPages. Simulate
    // disk now has 2 pages.
    invokeMock.mockResolvedValueOnce([
      { slug: "a", path: "/v/.codebus/wiki/modules/a.md", title: "A" },
      { slug: "b", path: "/v/.codebus/wiki/modules/b.md", title: "B" },
    ])
    useWikiStore.getState()._onTerminal({ run_id: "r-x" })
    // _onTerminal fires listPages fire-and-forget; wait one event-loop
    // tick for the awaited invoke promise + state set to settle.
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(Object.keys(useWikiStore.getState().pages).sort()).toEqual([
      "a",
      "b",
    ])
  })

  it("useWikiStore_onTerminal_invalidates_current_body_cache", async () => {
    invokeMock.mockResolvedValueOnce([])
    await useWikiStore.getState().listPages("/v")

    invokeMock.mockResolvedValueOnce("old body")
    await useWikiStore.getState().loadPage("/v", "stale")
    expect(useWikiStore.getState().body).toBe("old body")

    // Stub the listPages re-fire that _onTerminal triggers.
    invokeMock.mockResolvedValueOnce([])

    useWikiStore.getState()._onTerminal({ run_id: "r-x" })
    await new Promise((resolve) => setTimeout(resolve, 0))

    // Body cache for the currently-open page SHALL have been dropped
    // so the next loadPage refetches from disk.
    expect(useWikiStore.getState()._bodyCache["stale"]).toBeUndefined()
  })

  it("_onTerminal is a no-op when no vault path has been registered", async () => {
    // Fresh store — never called listPages.
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
      _currentVaultPath: null,
    })
    const callsBefore = invokeMock.mock.calls.length
    useWikiStore.getState()._onTerminal({ run_id: "r" })
    await Promise.resolve()
    expect(invokeMock.mock.calls.length).toBe(callsBefore)
  })

  // ---- Obsidian vault id probe (wiki-open-in-obsidian task 3.1) ----
  //
  // The store fetches the Obsidian vault id once when a vault's wiki loads
  // (folded into `listPages`) and caches it so WikiPreview can decide whether
  // to render `[Open in Obsidian]`. A null probe OR a probe error both cache
  // null (fail-soft) — the button stays hidden either way.

  function mockByCommand(map: Record<string, unknown>) {
    invokeMock.mockImplementation((command: string) => {
      if (command in map) {
        const v = map[command]
        return v instanceof Error ? Promise.reject(v) : Promise.resolve(v)
      }
      return Promise.resolve(undefined)
    })
  }

  it("listPages caches the Obsidian vault id when the probe returns one", async () => {
    mockByCommand({
      list_wiki_pages: [
        { slug: "a", path: "/v/.codebus/wiki/a.md", title: "A" },
      ],
      get_obsidian_vault_id: "abc123def456abcd",
    })
    await useWikiStore.getState().listPages("/v")
    expect(useWikiStore.getState().obsidianVaultId).toBe("abc123def456abcd")
  })

  it("listPages caches null when the vault is not registered in Obsidian", async () => {
    mockByCommand({
      list_wiki_pages: [],
      get_obsidian_vault_id: null,
    })
    await useWikiStore.getState().listPages("/v")
    expect(useWikiStore.getState().obsidianVaultId).toBeNull()
  })

  it("listPages caches null (fail-soft) and still lists pages when the probe errors", async () => {
    mockByCommand({
      list_wiki_pages: [
        { slug: "a", path: "/v/.codebus/wiki/a.md", title: "A" },
      ],
      get_obsidian_vault_id: new Error("obsidian.json parse failed"),
    })
    await useWikiStore.getState().listPages("/v")
    expect(useWikiStore.getState().obsidianVaultId).toBeNull()
    // The page list SHALL survive a probe failure.
    expect(Object.keys(useWikiStore.getState().pages)).toEqual(["a"])
  })

  it("reset clears the cached Obsidian vault id to null", async () => {
    mockByCommand({
      list_wiki_pages: [],
      get_obsidian_vault_id: "abc123def456abcd",
    })
    await useWikiStore.getState().listPages("/v")
    expect(useWikiStore.getState().obsidianVaultId).toBe("abc123def456abcd")
    useWikiStore.getState().reset()
    expect(useWikiStore.getState().obsidianVaultId).toBeNull()
  })
})
