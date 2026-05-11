/* global React */
const { useState } = React;

function VaultWorkspace() {

  // v1: at most one in-flight goal. Rest are completed (or failed) with timestamp only.
  const goals = [
    {
      status: 'running',
      name: 'Map the request lifecycle from HTTP entry to response',
      stream: 'reading src/server/router/index.ts … resolving middleware chain',
      live: { tokens: '4,218 tok' },
    },
    {
      status: 'done',
      name: 'How does the auth middleware compose with the router?',
      time: '14m ago',
    },
    {
      status: 'done',
      name: 'Identify the public plugin API surface',
      time: '1h ago',
    },
    {
      status: 'failed',
      name: 'Map the renderer/worker IPC protocol',
      time: '3h ago',
    },
    {
      status: 'done',
      name: 'Catalog the build/release pipeline (CI → notarize → publish)',
      time: 'yesterday',
    },
    {
      status: 'done',
      name: 'Walk through editor state management end-to-end',
      time: '2d ago',
    },
  ];

  return (
    <div className="cb-app" data-screen-label="01 Vault Workspace">
      <Sidebar active="goals" />

      {/* main */}
      <main className="cb-main">
        {/* Slim topbar = window-drag region in Tauri. No content; the
            screen title lives in the content header. */}
        <div className="cb-topbar" />

        <div className="cb-content">
          <div className="cb-row-head">
            <div>
              <h1 className="cb-h1">Goals</h1>
              <p className="cb-sub">LLM-driven exploration runs that build your wiki. Each goal posts to one or more wiki pages when it lands.</p>
            </div>
            <button className="cb-btn primary">
              <IconPlus /> New Goal <span className="cb-kshort">N</span>
            </button>
          </div>

          <div className="cb-sec-head">
            <span className="cb-sec-label">Recent</span>
            <span className="cb-sec-count">6 of 12</span>
          </div>

          <div className="cb-table">
            {goals.map((g, i) => (
              <div key={i} className={'cb-row' + (g.status === 'running' ? ' running' : '')}>
                <span className={'cb-r-status ' + g.status} />
                <div className="cb-r-title">
                  <span className="cb-r-name">{g.name}</span>
                  {g.stream && (
                    <span className="cb-r-stream">
                      {g.stream}<span className="caret" />
                    </span>
                  )}
                </div>
                {g.live ? (
                  <span className="cb-r-live">
                    streaming<span className="tok">· {g.live.tokens}</span>
                  </span>
                ) : (
                  <span className="cb-r-time">{g.time}</span>
                )}
                <span className="cb-r-kebab"><IconKebab /></span>
              </div>
            ))}
          </div>
        </div>
      </main>
    </div>
  );
}

window.VaultWorkspace = VaultWorkspace;
