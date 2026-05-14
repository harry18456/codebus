import { act, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const openUrlMock = vi.fn(() => Promise.resolve())
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: openUrlMock,
}))

import { ChatTranscript } from "./ChatTranscript"
import { useChatStore } from "@/store/chat"
import type { VerbEvent } from "@/lib/ipc"

const EMPTY_TOKENS = { input_tokens: 0, output_tokens: 0 }

function resetStore() {
  useChatStore.setState({
    sessionId: null,
    turns: [],
    activeTurn: null,
    tokensTotal: { ...EMPTY_TOKENS },
    promoteSuggestion: null,
    lastTranscript: null,
    lastSessionId: null,
  })
}

describe("ChatTranscript", () => {
  beforeEach(() => {
    resetStore()
    openUrlMock.mockClear()
  })

  afterEach(() => {
    resetStore()
  })

  it("renders user prompt + assistant tool one-liner + assistant text in a single completed turn", () => {
    const events: VerbEvent[] = [
      {
        kind: "stream",
        data: {
          kind: "tool_use",
          name: "Read",
          input: { file_path: "wiki/modules/auth.md" },
        },
      },
      { kind: "stream", data: { kind: "thought", text: "JWT-based..." } },
    ]
    useChatStore.setState({
      turns: [
        {
          userText: "auth 怎麼運作",
          events,
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:05Z",
        },
      ],
    })

    render(<ChatTranscript />)

    // User prompt rendered at top of turn block.
    const userBlock = screen.getByTestId("chat-turn-user")
    expect(userBlock).toHaveTextContent("auth 怎麼運作")

    // Tool one-liner reuses ActivityStreamItem (🛠️ Read · auth.md).
    const toolItem = screen.getByTestId("stream-tool-use")
    expect(toolItem).toHaveTextContent("Read")
    expect(toolItem).toHaveTextContent("auth.md")

    // Assistant text now renders via the markdown block (react-markdown).
    const markdown = screen.getByTestId("chat-assistant-markdown")
    expect(markdown).toHaveTextContent("JWT-based...")
  })

  it("renders two completed turns separated by a horizontal divider", () => {
    useChatStore.setState({
      turns: [
        {
          userText: "first question",
          events: [],
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:01Z",
        },
        {
          userText: "second question",
          events: [],
          startedAt: "2026-05-13T10:00:02Z",
          finishedAt: "2026-05-13T10:00:03Z",
        },
      ],
    })

    render(<ChatTranscript />)

    const turnBlocks = screen.getAllByTestId("chat-turn")
    expect(turnBlocks).toHaveLength(2)

    const dividers = screen.getAllByTestId("chat-turn-divider")
    // Exactly one divider between two turns (N-1 dividers for N turns).
    expect(dividers).toHaveLength(1)
  })

  // Auto-scroll was removed — initial sticky-on-mount implementation did
  // not actually pin in real DOM and the manual UX feedback (`/spectra-apply`
  // verify pass on v3-app-chat-cmdk) classified it as not-needed for v1.
  // Transcript falls back to standard browser scroll behavior; users scroll
  // manually if they want to follow the live stream.

  it("wiki markdown link click switches Wiki tab, loads page, and collapses chat", () => {
    // Seed a completed turn whose assistant text contains a wiki-markdown link.
    useChatStore.setState({
      expanded: true,
      turns: [
        {
          userText: "auth 怎麼運作",
          events: [
            {
              kind: "stream",
              data: {
                kind: "thought",
                text: "See [auth.md](wiki/modules/auth.md) for the full module.",
              },
            },
          ],
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:05Z",
        },
      ],
    })

    const onWikiLinkClick = vi.fn()
    render(<ChatTranscript onWikiLinkClick={onWikiLinkClick} />)

    const wikiLink = screen.getByTestId("chat-wiki-link")
    expect(wikiLink).toHaveTextContent("auth.md")

    fireEvent.click(wikiLink)

    // Spec: wiki tab is switched + loadPage invoked + widget collapsed.
    expect(onWikiLinkClick).toHaveBeenCalledTimes(1)
    expect(onWikiLinkClick).toHaveBeenCalledWith("wiki/modules/auth.md")
    expect(useChatStore.getState().expanded).toBe(false)
    // Spec: external opener NOT invoked.
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  it("external https link opens in browser via Tauri opener plugin without collapsing chat", async () => {
    useChatStore.setState({
      expanded: true,
      turns: [
        {
          userText: "找文件",
          events: [
            {
              kind: "stream",
              data: {
                kind: "thought",
                text: "Check [docs](https://example.com/foo).",
              },
            },
          ],
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:05Z",
        },
      ],
    })

    const onWikiLinkClick = vi.fn()
    render(<ChatTranscript onWikiLinkClick={onWikiLinkClick} />)

    const extLink = screen.getByTestId("chat-external-link")
    expect(extLink).toHaveTextContent("docs")
    // External link MUST remain an <a> tag (rendered with its href) for
    // assistive tech + still preventDefault-driven opener invocation.
    expect(extLink.tagName).toBe("A")

    await act(async () => {
      fireEvent.click(extLink)
      // Allow the dynamic-import + promise chain to flush.
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(openUrlMock).toHaveBeenCalledTimes(1)
    expect(openUrlMock).toHaveBeenCalledWith("https://example.com/foo")
    // Spec: tab + chat state untouched.
    expect(onWikiLinkClick).not.toHaveBeenCalled()
    expect(useChatStore.getState().expanded).toBe(true)
  })

  it("source code path renders as inert text with no click handler and no anchor href", () => {
    useChatStore.setState({
      turns: [
        {
          userText: "show me jwt",
          events: [
            {
              kind: "stream",
              data: {
                kind: "thought",
                text: "Look at [jwt.rs](src/auth/jwt.rs).",
              },
            },
          ],
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:05Z",
        },
      ],
    })

    const onWikiLinkClick = vi.fn()
    render(<ChatTranscript onWikiLinkClick={onWikiLinkClick} />)

    const inert = screen.getByTestId("chat-inert-link")
    expect(inert).toHaveTextContent("jwt.rs")
    // Spec: no <a> with non-empty href.
    expect(inert.tagName).not.toBe("A")
    expect(inert).not.toHaveAttribute("href")

    // Clicking the inert element MUST NOT invoke wiki nav or external opener.
    fireEvent.click(inert)
    expect(onWikiLinkClick).not.toHaveBeenCalled()
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  // -------------------------------------------------------------------------
  // Promote pill (task 5.3) — spec `Promote to Goal Pill on Assistant Message`
  // -------------------------------------------------------------------------

  it("renders promote pill on the assistant message when a PromoteSuggestion is active", () => {
    // Active turn carrying a promote_suggestion lifecycle event — store has
    // already stamped promoteSuggestion with turnIndex pointing at the slot
    // the active turn will occupy once it finalizes (turns.length === 0).
    useChatStore.setState({
      turns: [],
      activeTurn: {
        vaultPath: "/vault",
        userText: "tell me about auth",
        runId: "chat-1",
        events: [
          {
            kind: "stream",
            data: { kind: "thought", text: "JWT-based authentication..." },
          },
          {
            kind: "lifecycle",
            data: {
              kind: "promote_suggestion",
              reason: "auth + JWT 適合寫成 wiki",
            },
          },
        ],
        cancelling: false,
        startedAt: "2026-05-14T10:00:00Z",
      },
      promoteSuggestion: {
        reason: "auth + JWT 適合寫成 wiki",
        turnIndex: 0,
      },
    })

    render(<ChatTranscript vaultPath="/vault" />)

    const pill = screen.getByTestId("promote-pill")
    expect(pill).toHaveTextContent("Promote to goal: auth + JWT 適合寫成 wiki")
    expect(pill).toHaveTextContent("Dismiss")
    // Pill MUST live inside the active turn's assistant message block, not
    // hanging off the transcript root.
    const activeTurn = screen.getByTestId("chat-turn-active")
    expect(activeTurn.contains(pill)).toBe(true)
  })

  it("clicking Promote calls acceptPromoteSuggestion and collapses + routes on success", async () => {
    const acceptPromote = vi.fn().mockResolvedValue("2026-05-14T10-20-30Z")
    useChatStore.setState({
      expanded: true,
      turns: [
        {
          userText: "Q1",
          events: [
            { kind: "stream", data: { kind: "thought", text: "A1" } },
          ],
          startedAt: "2026-05-14T10:00:00Z",
          finishedAt: "2026-05-14T10:00:05Z",
        },
      ],
      activeTurn: null,
      promoteSuggestion: { reason: "topic worth wiki", turnIndex: 0 },
      acceptPromoteSuggestion: acceptPromote,
    })

    const onPromoteSuccess = vi.fn()
    render(
      <ChatTranscript
        vaultPath="/vault"
        onPromoteSuccess={onPromoteSuccess}
      />,
    )

    fireEvent.click(screen.getByRole("button", { name: /Promote to goal/ }))

    await waitFor(() => {
      expect(acceptPromote).toHaveBeenCalledTimes(1)
    })
    expect(acceptPromote).toHaveBeenCalledWith("/vault")
    // acceptPromoteSuggestion is the abstraction; spec scenario 2's exact
    // transcript-string assertion lives in the chat store unit test (this
    // component test would just be re-asserting store internals).

    await waitFor(() => {
      expect(onPromoteSuccess).toHaveBeenCalledWith("2026-05-14T10-20-30Z")
    })
  })

  it("clicking Promote surfaces inline error and keeps pill clickable when goal already active", async () => {
    const acceptPromote = vi.fn().mockRejectedValue({
      kind: "invalid",
      field: "active_runs",
      message: "Another goal is running",
    })
    useChatStore.setState({
      turns: [
        {
          userText: "Q1",
          events: [],
          startedAt: "2026-05-14T10:00:00Z",
          finishedAt: "2026-05-14T10:00:05Z",
        },
      ],
      activeTurn: null,
      promoteSuggestion: { reason: "topic worth wiki", turnIndex: 0 },
      acceptPromoteSuggestion: acceptPromote,
    })

    const onPromoteSuccess = vi.fn()
    render(
      <ChatTranscript
        vaultPath="/vault"
        onPromoteSuccess={onPromoteSuccess}
      />,
    )

    fireEvent.click(screen.getByRole("button", { name: /Promote to goal/ }))

    const errEl = await screen.findByTestId("promote-error")
    expect(errEl).toHaveTextContent("Another goal is running")
    // Pill MUST still be in the DOM + retryable.
    expect(screen.getByTestId("promote-pill")).toBeInTheDocument()
    expect(onPromoteSuccess).not.toHaveBeenCalled()
    // Second click MUST re-invoke acceptPromoteSuggestion (no disabled state
    // that blocks retry after a transient active-runs collision).
    fireEvent.click(screen.getByRole("button", { name: /Promote to goal/ }))
    await waitFor(() => {
      expect(acceptPromote).toHaveBeenCalledTimes(2)
    })
  })

  it("clicking Dismiss removes the pill and prevents re-emit on the same message", () => {
    const dismissPromote = vi.fn(() => {
      useChatStore.setState({ promoteSuggestion: null })
    })
    useChatStore.setState({
      turns: [
        {
          userText: "Q1",
          events: [],
          startedAt: "2026-05-14T10:00:00Z",
          finishedAt: "2026-05-14T10:00:05Z",
        },
      ],
      activeTurn: null,
      promoteSuggestion: { reason: "topic worth wiki", turnIndex: 0 },
      dismissPromoteSuggestion: dismissPromote,
    })

    const { rerender } = render(<ChatTranscript vaultPath="/vault" />)
    expect(screen.getByTestId("promote-pill")).toBeInTheDocument()

    fireEvent.click(screen.getByRole("button", { name: /Dismiss/ }))

    expect(dismissPromote).toHaveBeenCalledTimes(1)
    expect(useChatStore.getState().promoteSuggestion).toBeNull()
    expect(screen.queryByTestId("promote-pill")).toBeNull()

    // Re-rendering the same transcript MUST NOT bring the pill back — it is
    // gated entirely on store.promoteSuggestion non-null + turnIndex match.
    rerender(<ChatTranscript vaultPath="/vault" />)
    expect(screen.queryByTestId("promote-pill")).toBeNull()
  })

  it("does not render promote pill on turns whose index does not match turnIndex", () => {
    useChatStore.setState({
      turns: [
        {
          userText: "Q1",
          events: [],
          startedAt: "2026-05-14T10:00:00Z",
          finishedAt: "2026-05-14T10:00:01Z",
        },
        {
          userText: "Q2",
          events: [],
          startedAt: "2026-05-14T10:00:02Z",
          finishedAt: "2026-05-14T10:00:03Z",
        },
      ],
      activeTurn: null,
      // turnIndex points at turn 1 (Q2) only.
      promoteSuggestion: { reason: "Q2 worth wiki", turnIndex: 1 },
    })

    render(<ChatTranscript vaultPath="/vault" />)

    const pills = screen.getAllByTestId("promote-pill")
    expect(pills).toHaveLength(1)
    // The pill must live inside the second turn's block, not the first.
    const turnBlocks = screen.getAllByTestId("chat-turn")
    expect(turnBlocks[0].querySelector("[data-testid='promote-pill']")).toBeNull()
    expect(turnBlocks[1].contains(pills[0])).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Onboarding hint (task 5.4) — spec `Chat Onboarding Hint and Placeholder`
  // -------------------------------------------------------------------------

  describe("onboarding hint", () => {
    beforeEach(() => {
      localStorage.clear()
    })

    afterEach(() => {
      localStorage.clear()
    })

    it("renders the onboarding hint whenever the transcript is empty", () => {
      // Empty transcript (turns + activeTurn both empty) → hint renders
      // inside the transcript region with the documented substrings,
      // regardless of any prior onboarded flag.
      useChatStore.setState({ turns: [], activeTurn: null })

      render(<ChatTranscript vaultPath="/vault/new" />)

      const hint = screen.getByTestId("chat-onboarding-hint")
      // Spec substrings: en hint MUST contain both "AI will suggest" and
      // "ask AI to promote" so the user understands both promote paths.
      expect(hint).toHaveTextContent("AI will suggest")
      expect(hint).toHaveTextContent("ask AI to promote")
      // Hint lives inside the transcript region (not floating off the widget).
      expect(
        screen.getByTestId("chat-transcript").contains(hint),
      ).toBe(true)
    })

    it("re-shows the hint after + New chat clears the transcript", () => {
      // Simulate a prior session whose hint was already shown (legacy
      // localStorage flag still set on this vault) followed by `+ New chat`
      // emptying the transcript. The hint MUST re-appear because the user
      // wants every fresh conversation to reaffirm promote mechanics.
      const vaultPath = "/vault/seen"
      useChatStore.getState().markOnboarded(vaultPath)
      useChatStore.setState({ turns: [], activeTurn: null })

      render(<ChatTranscript vaultPath={vaultPath} />)

      expect(screen.getByTestId("chat-onboarding-hint")).toBeInTheDocument()
    })

    it("hides the hint as soon as there is an active turn or completed turn", () => {
      useChatStore.setState({
        turns: [],
        activeTurn: {
          vaultPath: "/vault/active",
          userText: "asking",
          runId: "chat-active-1",
          events: [],
          cancelling: false,
          startedAt: "2026-05-14T10:20:30Z",
        },
      })

      render(<ChatTranscript vaultPath="/vault/active" />)

      expect(screen.queryByTestId("chat-onboarding-hint")).toBeNull()
    })
  })

  it("plain text wiki mention is not auto-linked", () => {
    useChatStore.setState({
      turns: [
        {
          userText: "auth",
          events: [
            {
              kind: "stream",
              data: {
                kind: "thought",
                text: "see wiki/modules/auth.md for details",
              },
            },
          ],
          startedAt: "2026-05-13T10:00:00Z",
          finishedAt: "2026-05-13T10:00:05Z",
        },
      ],
    })

    const onWikiLinkClick = vi.fn()
    render(<ChatTranscript onWikiLinkClick={onWikiLinkClick} />)

    // No clickable element of any flavor should appear.
    expect(screen.queryByTestId("chat-wiki-link")).toBeNull()
    expect(screen.queryByTestId("chat-external-link")).toBeNull()
    expect(screen.queryByTestId("chat-inert-link")).toBeNull()

    // The text is still rendered in the markdown block as inert prose.
    const markdown = screen.getByTestId("chat-assistant-markdown")
    expect(markdown).toHaveTextContent("see wiki/modules/auth.md for details")
  })
})
