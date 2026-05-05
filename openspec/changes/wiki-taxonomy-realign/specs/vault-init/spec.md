## MODIFIED Requirements

### Requirement: Initialize .codebus/ vault structure under user repo

When invoked with `--repo <path>` and no `--goal` or `--query`, the system SHALL create the `.codebus/` vault under the given path containing all subdirectories required by the wiki workflow.

#### Scenario: Fresh init creates all expected paths

- **WHEN** the user runs `codebus --repo /path/to/myrepo` and `/path/to/myrepo/.codebus/` does not yet exist
- **THEN** the system creates `/path/to/myrepo/.codebus/` with subdirectories `raw/`, `raw/code/`, `wiki/`, `wiki/concepts/`, `wiki/entities/`, `wiki/modules/`, `wiki/processes/`, `wiki/synthesis/`, `output/`, and files `CLAUDE.md`, `goals.jsonl`, `.gitignore` (Karpathy-style 5-folder knowledge structure; folder = page `type` enum). The system SHALL NOT create `wiki/goals/`.

#### Scenario: Internal .gitignore excludes lock and raw code

- **WHEN** init writes `.codebus/.gitignore`
- **THEN** the file contains entries for `.lock` and `raw/code/` so that lock files and the codebase mirror are not tracked by the nested git repo
