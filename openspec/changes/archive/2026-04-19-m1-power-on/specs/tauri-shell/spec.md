## ADDED Requirements

### Requirement: Tauri 2.0 application shell

The `tauri/` project SHALL produce a runnable Tauri 2.0 desktop application that hosts the Nuxt3 frontend.

#### Scenario: Dev mode launches

- **WHEN** `cargo tauri dev` is run from the `tauri/` directory
- **THEN** a Tauri window MUST open and render the Nuxt3 frontend at an internal URL

#### Scenario: Window displays application title

- **WHEN** the Tauri window opens
- **THEN** the window title MUST be `CodeBus`

### Requirement: Nuxt3 landing page

The `web/` project SHALL render a landing page that proves the frontend is wired to the Tauri shell.

#### Scenario: Landing page renders literal text

- **WHEN** the Tauri window opens in dev mode
- **THEN** the visible DOM MUST contain the literal text `CodeBus`

#### Scenario: Tailwind is configured

- **WHEN** the landing page is inspected
- **THEN** at least one Tailwind utility class MUST be applied to a rendered element, proving `@nuxtjs/tailwindcss` is active

### Requirement: Filesystem scope restricts access

Tauri's filesystem capability SHALL be configured with `fs.scope` so that the frontend can only reach approved workspace paths, per `docs/tool-sandbox.md §七`.

#### Scenario: Unapproved path denied

- **WHEN** the frontend attempts `fs.readTextFile` on a path outside the configured scope
- **THEN** Tauri MUST reject the call and MUST NOT return file contents

#### Scenario: Approved workspace path allowed

- **WHEN** the frontend attempts `fs.readTextFile` on a path inside the configured scope
- **THEN** Tauri MUST return the file contents
