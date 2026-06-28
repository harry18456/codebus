## ADDED Requirements

### Requirement: Tool descriptions convey the cross-project wiki-library use case

The four query tools' MCP descriptions SHALL convey not only how to call each tool but when an agent reaches for it: the server exposes a library of codebus-generated wikis for codebases the user has indexed, usable as cross-project reference (applying a pattern from one indexed codebase to another, answering "how does X work" against an indexed codebase), to be cross-checked against current source rather than trusted as ground truth. The `vault_list` description SHALL frame the set of vaults as that cross-project library. The `wiki_search` and `wiki_list` descriptions SHALL convey when to use them (when a cross-project reference or a known pattern from an indexed codebase helps, or when asked). These framing additions SHALL NOT remove the existing mechanical guidance: `wiki_search` SHALL still instruct callers to pass a single keyword rather than a full sentence, and `wiki_read` SHALL still describe character pagination. Descriptions SHALL remain concise — an agent reads every tool description on each call — so the framing is one or two sentences, not prose.

#### Scenario: vault_list description frames the cross-project library

- **WHEN** an agent reads the `vault_list` tool description
- **THEN** the description SHALL convey that the listed vaults are a library of codebus wikis across codebases the user has indexed (a cross-project reference), not merely a bare list of vaults

#### Scenario: wiki_search description adds framing but keeps the keyword instruction

- **WHEN** an agent reads the `wiki_search` tool description
- **THEN** the description SHALL convey that it searches across the indexed codebase wikis AND SHALL still instruct the caller to pass a single keyword rather than a full sentence
