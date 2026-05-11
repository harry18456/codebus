/* global React */
function TokensCard() {
  const colors = [
    ['--bg',           '#0A0A0A', 'bg'],
    ['--bg-raised',    '#111111', 'raised'],
    ['--bg-hover',     '#161616', 'hover'],
    ['--bg-active',    '#1A1A1A', 'active'],
    ['--border',       '#1F1F1F', 'border'],
    ['--border-strong','#2A2A2A', 'border-2'],
    ['--fg',           '#E5E5E5', 'fg'],
    ['--fg-secondary', '#8A8A8A', 'fg/2'],
    ['--fg-tertiary',  '#5A5A5A', 'fg/3'],
    ['--accent',       '#F5A623', 'accent'],
    ['--success',      '#4ADE80', 'ok'],
    ['--error',        '#F87171', 'err'],
  ];
  return (
    <div className="cb-tokens" data-screen-label="00 Tokens">
      <h3>Color</h3>
      {colors.map(([v, hex, name]) => (
        <div key={v} className="cb-swatch">
          <div className="chip" style={{ background: 'var(' + v + ')' }} />
          <div className="meta"><span className="name">{name}</span><span className="val">{hex}</span></div>
        </div>
      ))}

      <h3 style={{ marginTop: 8 }}>Type · Inter (UI) + JetBrains Mono</h3>
      <div className="cb-type-row" style={{ gridColumn: '1 / -1' }}>
        <span className="label">H1 · 18/600</span>
        <span style={{ fontSize: 18, fontWeight: 600, letterSpacing: '-0.015em' }}>Map the request lifecycle</span>
      </div>
      <div className="cb-type-row" style={{ gridColumn: '1 / -1' }}>
        <span className="label">Body · 13/400</span>
        <span style={{ fontSize: 13 }}>LLM-driven exploration runs that build your wiki.</span>
      </div>
      <div className="cb-type-row" style={{ gridColumn: '1 / -1' }}>
        <span className="label">Meta · 11/400</span>
        <span style={{ fontSize: 11, color: 'var(--fg-tertiary)' }}>6 of 12 · just now · 14m ago</span>
      </div>
      <div className="cb-type-row" style={{ gridColumn: '1 / -1' }}>
        <span className="label">Mono · 12/400</span>
        <span className="mono" style={{ fontSize: 12 }}>~/code/linear-clone · src/server/**/*.ts</span>
      </div>

      <h3 style={{ marginTop: 8 }}>Controls</h3>
      <div style={{ gridColumn: '1 / -1', display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center', padding: 14, border: '1px solid var(--border)', borderRadius: 6, background: 'var(--bg-raised)' }}>
        <button className="cb-btn primary"><IconPlus /> New Goal</button>
        <button className="cb-btn"><IconRefresh /> Re-run</button>
        <button className="cb-btn ghost"><IconFilter /> All statuses</button>
        <span className="cb-tag running">running</span>
        <span className="cb-tag queued">queued</span>
        <span className="cb-tag done">done</span>
        <span className="cb-tag failed">failed</span>
        <div className="cb-kbd"><kbd>⌘</kbd><kbd>K</kbd></div>
      </div>
    </div>
  );
}
window.TokensCard = TokensCard;
