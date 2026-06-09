## MODIFIED Requirements

### Requirement: Wikilink Plain and Citation Style Variants

The wikilink renderer SHALL apply exactly two distinct visual variants depending on rendering context:

- **Plain wikilink variant** -- Used for resolvable wikilinks inside a wiki body. The element SHALL carry the literal CSS class name `plain-wikilink`, SHALL render with the accent foreground color in its default state, SHALL use the proportional body font, and SHALL NOT render the raw `[[ ]]` bracket syntax. The default state SHALL NOT use a dashed underline. On hover and keyboard focus the element SHALL expose an accent underline at a 3px underline offset. Under `prefers-reduced-motion: reduce`, the hover/focus color and underline changes SHALL be applied instantly without a CSS transition.
- **Citation wikilink variant** -- Used inside citation blocks and citation-like inline provenance text such as quiz explanations and chat bubble citations. The element SHALL carry the literal CSS class name `cite-link` and SHALL render in a monospace font with the accent foreground color and a dashed underline at a 3px offset.

Unresolvable wikilinks (slugs that are not present in the wiki page index) SHALL continue to render as a dimmed `cursor-not-allowed` element with the `Page not found` tooltip; no new state SHALL be introduced beyond the existing resolvable / unresolvable distinction. The wikilink renderer SHALL NOT track or visualize a `visited` state.

#### Scenario: Resolvable body wikilink uses plain-wikilink class

- **WHEN** a resolvable `[[slug]]` is rendered inside a wiki body
- **THEN** the rendered anchor element has the literal CSS class `plain-wikilink` in its class list AND the anchor uses the accent foreground token by default AND the rendered label is the resolved page title when known or the slug fallback when unknown AND the raw `[[slug]]` bracket syntax is not visible

#### Scenario: Body wikilink remains visually distinct from citation wikilink

- **WHEN** a wiki body link and a quiz or chat citation link are rendered on the same page
- **THEN** the body link uses `plain-wikilink` with proportional accent text and no dashed underline in its default state AND the citation link uses `cite-link` with monospace accent text and a dashed underline

#### Scenario: Citation wikilink uses cite-link class

- **WHEN** a wikilink is rendered inside a quiz citation block, quiz explanation citation, or a chat-bubble citation block
- **THEN** the rendered anchor element has the literal CSS class `cite-link` in its class list AND uses the monospace / accent / dashed-underline styling

#### Scenario: Reduced motion suppresses hover transition

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** hovering or focusing a `plain-wikilink` element changes the color and underline presentation instantly without a CSS transition

#### Scenario: No visited state is rendered

- **WHEN** the user navigates to a wikilink target and later returns to a page that links to that target
- **THEN** the wikilink renders with the same `plain-wikilink` or `cite-link` styling as a never-clicked link AND no visited-state styling is applied

### Requirement: Quiz Answering and Summary

The answering view SHALL present one question per screen with four choices. After the user selects a choice and submits, the system SHALL reveal whether it was correct by comparing the selection to the quiz markdown `Answer` field client-side (no agent spawn) and SHALL show the `Explanation`. After the final question, a summary SHALL display the score and a pass/fail outcome computed client-side using `app.quiz.pass_threshold`. The threshold value SHALL be sourced from the application settings store (the same `app.quiz.pass_threshold` key the Settings modal binds); it SHALL NOT be a hardcoded component constant. When the `app.quiz.pass_threshold` key is absent the value SHALL default to 80; changing the setting SHALL change the summary pass/fail boundary on the next finished quiz.

The revealed `Explanation` SHALL render each of its `[[slug]]` wikilink citations as an interactive wikilink, on BOTH correct and incorrect submissions (and likewise wherever the per-question explanation is shown in the Review view). A citation whose slug resolves to an existing wiki page SHALL be activatable; activating it SHALL navigate the workspace to that wiki page (the same navigation as selecting the page from the wiki tree). A citation whose slug does not resolve SHALL render in the standard unresolved-wikilink presentation and SHALL NOT be activatable. The system SHALL NOT render a separate `[← Back to wiki page]` affordance; the explanation's per-question citations are the source-navigation mechanism.

The answering view and the completed-attempt Review view SHALL render quiz text fragments through the same inline markdown renderer for all question stems, choice labels, and explanations. This renderer SHALL support only inline code, strong emphasis, emphasis, and existing `[[slug]]` wikilink citation parsing. It SHALL NOT render block markdown constructs in quiz text fragments, including code fences, tables, headings, lists, or blockquotes. Unsupported block syntax SHALL NOT create block-level quiz layout.

The answering view SHALL persist progress to the attempt's progress sidecar (see capability `quiz`) on every submission AND on every Next via the `write_quiz_progress` command: each submission appends/updates the answered question with the user's `selected` choice and `correct` boolean, sets `status: in_progress`, and sets `cursor` to `{ q: <that question>, revealed: true }`; submitting the final question SHALL set `status: completed` and `completed_at`; pressing Next SHALL set `cursor` to `{ q: <next question>, revealed: false }` (answers unchanged, `status: in_progress`). When an attempt is opened that already has an in-progress sidecar with a `cursor`, the answering view SHALL restore exactly that position: question `cursor.q`, shown in its submitted state (stored `selected` + verdict + `Explanation`) when `cursor.revealed` is true, or as a blank unanswered question when false. When the sidecar has no `cursor` (legacy), the answering view SHALL instead restore the last answered question (highest 1-based number in `answers`) in its submitted state. It SHALL NOT restart at question 1 for an in-progress attempt. Persistence SHALL NOT spawn an agent.

#### Scenario: Correct answer revealed without spawn

- **WHEN** the user submits the choice matching the question's `Answer` field
- **THEN** the system SHALL mark it correct AND show the `Explanation` AND SHALL NOT spawn an agent to grade

#### Scenario: Explanation citations render as navigable wikilinks on both outcomes

- **GIVEN** a question whose `Explanation` cites `[[auth-middleware-verification]]` and that slug resolves to an existing wiki page
- **WHEN** the user submits an answer (whether correct or incorrect) and the `Explanation` is revealed
- **THEN** the citation SHALL render as an activatable wikilink AND activating it SHALL navigate the workspace to the `auth-middleware-verification` wiki page AND no `[← Back to wiki page]` affordance SHALL be rendered

#### Scenario: Unresolvable citation is not activatable

- **GIVEN** a question whose `Explanation` cites `[[no-such-page]]` and that slug resolves to no wiki page
- **WHEN** the `Explanation` is revealed
- **THEN** the citation SHALL render in the standard unresolved-wikilink presentation AND SHALL NOT navigate anywhere when activated

#### Scenario: Inline markdown renders in answering text fragments

- **GIVEN** a quiz question stem containing `` `codebus-core` `` and `**Rust**`, a choice containing `*workspace*`, and an explanation containing `` `read_wiki_page` `` plus `[[desktop-app-workspace]]`
- **WHEN** the answering view renders the question and later reveals the explanation
- **THEN** the stem, choice, and explanation render inline code, strong text, emphasis, and the wikilink citation semantically AND the raw backtick, double-asterisk, and single-asterisk delimiters are not visible as formatting markers

#### Scenario: Inline markdown renders in review text fragments

- **GIVEN** a completed quiz attempt whose stem, choices, and explanation contain inline code, strong text, emphasis, and `[[desktop-app-workspace]]`
- **WHEN** the Review view renders the completed attempt
- **THEN** the stem, choices, and explanation render the same inline markdown subset as the answering view AND the wikilink citation remains resolvable through the wiki page index

#### Scenario: Block markdown is not supported in quiz text fragments

- **GIVEN** a quiz stem, choice, or explanation contains markdown fence, table, heading, list, or blockquote syntax
- **WHEN** the answering view or Review view renders that text fragment
- **THEN** the renderer SHALL NOT create code block, table, heading, list, or blockquote DOM for that quiz text fragment

#### Scenario: Summary applies pass threshold

- **GIVEN** `app.quiz.pass_threshold` is 80
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a passing outcome

#### Scenario: Changing the threshold setting changes the outcome

- **GIVEN** `app.quiz.pass_threshold` is set to 90 in the settings store
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a failing outcome

#### Scenario: Each submission persists progress

- **WHEN** the user submits an answer to a question
- **THEN** the system SHALL call `write_quiz_progress` recording that question's `selected` and `correct` with `status: in_progress` (or `completed` on the final question) AND SHALL NOT spawn an agent

#### Scenario: Resume restores the exact cursor position (advanced past the answered question)

- **GIVEN** an attempt whose sidecar has answers for questions 1-3 of 5, `status: in_progress`, and `cursor: { q: 4, revealed: false }` (the user submitted Q3 then pressed Next, then left)
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL show question 4 as a blank unanswered question AND SHALL NOT show question 3's submitted state

#### Scenario: Resume restores the exact cursor position (not yet advanced)

- **GIVEN** an attempt whose sidecar has answers for questions 1-3 of 5, `status: in_progress`, and `cursor: { q: 3, revealed: true }` (the user submitted Q3 and left without pressing Next)
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL restore question 3 in its submitted state -- the stored `selected` choice for question 3, its verdict, and its `Explanation`

#### Scenario: Legacy sidecar without a cursor falls back to last answered

- **GIVEN** an attempt whose sidecar has answers for questions 1 and 2 of 5, `status: in_progress`, and NO `cursor` field
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL restore question 2 (the last answered) in its submitted state AND SHALL NOT restart at question 1

## ADDED Requirements

### Requirement: Wiki Markdown Code Block Highlighting

The wiki markdown preview SHALL apply syntax highlighting to markdown code blocks using a rehype-based highlighter. Fenced code blocks with a recognized language info string SHALL preserve the `language-<name>` class and SHALL contain highlighted token elements or token classes rather than a single raw text-only code node. The code block container SHALL continue to use the app's existing pre box shape, spacing, border, and sunken background.

The syntax theme SHALL be a dark, existing highlight theme adapted to the app. Theme selectors SHALL be scoped to the wiki preview container so highlight token styles do not leak into chat, quiz, or other markdown surfaces. Theme background color SHALL NOT override the existing app pre box background.

Inline code in wiki markdown SHALL retain the existing inline chip presentation and SHALL NOT be rendered as a highlighted code block.

#### Scenario: Rust fenced code block renders highlighted tokens

- **WHEN** a wiki page body contains a fenced code block with the `rust` language info string
- **THEN** the wiki preview renders the code block with `language-rust` and highlight token descendants or token classes inside the code element

#### Scenario: Code block theme is scoped to wiki preview

- **WHEN** wiki preview highlighting styles are loaded
- **THEN** highlight token styles apply only under the wiki preview container AND a non-wiki surface using an `hljs` class outside that container does not receive the wiki preview highlight theme

#### Scenario: Code block background remains app-owned

- **WHEN** a highlighted wiki code block is rendered
- **THEN** the pre container keeps the app's sunken background, border, padding, and overflow behavior AND the highlight theme does not replace the pre background

##### Example: highlighted rust pre box

- **GIVEN** the wiki markdown body contains ```` ```rust\nfn main() {}\n``` ````
- **WHEN** the wiki preview renders that code block
- **THEN** the `<pre>` element keeps the app pre box styling while the nested code tokens receive highlight classes

#### Scenario: Inline code remains an inline chip

- **WHEN** a wiki paragraph contains inline code such as `` `codebus-core` ``
- **THEN** the wiki preview renders it as inline code using the existing chip presentation AND does not render it as a highlighted block
