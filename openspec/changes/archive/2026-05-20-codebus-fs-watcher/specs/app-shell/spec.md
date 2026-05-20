## ADDED Requirements

### Requirement: Lobby Subscribes To Vault List Watcher

The Lobby SHALL subscribe to the `vault-list-changed` Tauri event (defined by the `fs-watcher` capability) via the `useWatcherEvent` hook and SHALL invoke `useVaultListStore.load()` whenever the event fires. The subscription SHALL be active for the entire lifetime of the Lobby component and SHALL be cleaned up on unmount.

#### Scenario: External vault add refreshes Lobby

- **GIVEN** the Lobby is displayed AND a vault watcher monitors `~/.codebus/app-state.json`
- **WHEN** an external process appends a new vault entry to `~/.codebus/app-state.json`
- **THEN** the Lobby SHALL re-render with the new vault card visible within 400 ms (200 ms debounce window plus scheduling slack)

#### Scenario: Subscription is cleaned up on unmount

- **GIVEN** the Lobby has subscribed to `vault-list-changed`
- **WHEN** the Lobby unmounts (user opens a vault and enters Workspace)
- **THEN** the `useWatcherEvent` cleanup function SHALL be invoked AND no further Lobby re-render SHALL be triggered by subsequent `vault-list-changed` events while the Lobby is unmounted

### Requirement: Workspace Manages Per-Vault Watcher Lifecycle

The Workspace component SHALL invoke `start_vault_watcher(vault_path)` on mount and `stop_vault_watcher(vault_path)` on unmount, binding the per-vault watcher's lifecycle to the Workspace as defined by the `fs-watcher` capability. Switching from one vault's Workspace to another's SHALL release the prior vault's watcher before starting the new one.

#### Scenario: Workspace mount starts the watcher for the open vault

- **WHEN** the user opens vault V from the Lobby and the Workspace component mounts
- **THEN** `start_vault_watcher(V)` SHALL be invoked exactly once before any watcher-driven refresh is expected to occur

#### Scenario: Workspace unmount stops the watcher

- **WHEN** the user returns from Workspace to Lobby
- **THEN** `stop_vault_watcher(V)` SHALL be invoked for the previously open vault V

#### Scenario: Vault switch releases the prior watcher

- **GIVEN** the Workspace is mounted for vault V1
- **WHEN** the user switches to vault V2 (Workspace remounts)
- **THEN** `stop_vault_watcher(V1)` SHALL be invoked AND then `start_vault_watcher(V2)` SHALL be invoked, in that order
