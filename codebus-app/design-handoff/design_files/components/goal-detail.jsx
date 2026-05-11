/* global React */
const { useState: useStateGoal } = React;

// File-type icons. Minimal: read = page glyph, write = pencil.
const GFileRead = () => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z"/><path d="M14 3v5h5"/>
  </svg>
);
const GFileWrite = () => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11 4H5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h13a2 2 0 0 0 2-2v-6"/><path d="m18 2 4 4-11 11H7v-4Z"/>
  </svg>
);
const GSpinner = () => (
  <svg className="cb-spinner" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
    <circle cx="12" cy="12" r="9" strokeDasharray="14 32" />
  </svg>
);
const GChev = ({ open }) => (
  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
       style={{ transform: open ? 'rotate(0deg)' : 'rotate(-90deg)', transition: 'transform .15s' }}>
    <path d="m6 9 6 6 6-6"/>
  </svg>
);

function TimelineSection({ title, count, rows, open = true, trailing }) {
  const [isOpen, setOpen] = useStateGoal(open);
  return (
    <section className="cb-tl-section">
      <header className="cb-tl-section-head" onClick={() => setOpen(o => !o)}>
        <GChev open={isOpen} />
        <span className="cb-tl-section-title">{title}</span>
        <span className="cb-tl-section-count">{count}</span>
      </header>
      {isOpen && (
        <div className="cb-tl-rows">
          {rows.map((r, i) => (
            <div key={i} className="cb-tl-row">
              <span className="cb-tl-icon" style={{ color: r.kind === 'write' ? 'var(--accent)' : 'var(--fg-tertiary)' }}>
                {r.kind === 'write' ? <GFileWrite /> : <GFileRead />}
              </span>
              <span className="cb-tl-path mono">{r.path}</span>
              {r.badge && <span className={'cb-mini-badge ' + r.badge}>{r.badge}</span>}
              <span className="cb-tl-time mono">{r.time}</span>
            </div>
          ))}
          {trailing}
        </div>
      )}
    </section>
  );
}

function StreamLog({ defaultOpen = false, lines }) {
  const [open, setOpen] = useStateGoal(defaultOpen);
  return (
    <section className="cb-collapse">
      <header className="cb-collapse-head" onClick={() => setOpen(o => !o)}>
        <GChev open={open} />
        <span className="cb-tl-section-title">{open ? 'stream log' : 'stream log'}</span>
        <span className="cb-tl-section-count mono">{lines.length} events</span>
      </header>
      {open && (
        <pre className="cb-stream-log">
{lines.map((l, i) => (
  <code key={i} className="cb-stream-line">
    <span className="cb-stream-ts">{l.ts}</span>
    <span className={'cb-stream-tag ' + l.tag}>{l.tag}</span>
    <span className="cb-stream-msg">{l.msg}</span>
  </code>
))}
        </pre>
      )}
    </section>
  );
}

function GoalDetail({ state = 'running' }) {
  const isRunning = state === 'running';
  const goalText = '搞懂 auth 模組怎麼運作';

  const readingRows = [
    { kind: 'read',  path: 'src/auth/middleware.ts',  time: '+2s' },
    { kind: 'read',  path: 'src/auth/jwt.ts',         time: '+3s' },
    { kind: 'read',  path: 'src/auth/session.ts',     time: '+4s' },
  ];
  const writingRows = isRunning
    ? [
        { kind: 'write', path: 'modules/auth-middleware.md',     badge: 'new',     time: '+8s' },
        { kind: 'write', path: 'concepts/jwt-token-lifecycle.md', badge: 'new',     time: '+12s' },
        { kind: 'write', path: 'index.md',                        badge: 'updated', time: '+14s' },
      ]
    : [
        { kind: 'write', path: 'modules/auth-middleware.md',     badge: 'new',     time: '+8s' },
        { kind: 'write', path: 'concepts/jwt-token-lifecycle.md', badge: 'new',     time: '+12s' },
        { kind: 'write', path: 'index.md',                        badge: 'updated', time: '+14s' },
      ];

  const logLines = [
    { ts: '00:00.142', tag: 'goal',  msg: 'received goal "搞懂 auth 模組怎麼運作"' },
    { ts: '00:01.880', tag: 'plan',  msg: 'plan: read auth surface → identify entry points → write 2 pages' },
    { ts: '00:02.314', tag: 'read',  msg: 'src/auth/middleware.ts (412 lines)' },
    { ts: '00:03.061', tag: 'read',  msg: 'src/auth/jwt.ts (188 lines)' },
    { ts: '00:04.220', tag: 'read',  msg: 'src/auth/session.ts (256 lines)' },
    { ts: '00:08.711', tag: 'write', msg: 'modules/auth-middleware.md +1.4k bytes' },
    { ts: '00:12.040', tag: 'write', msg: 'concepts/jwt-token-lifecycle.md +2.1k bytes' },
    { ts: '00:14.298', tag: 'write', msg: 'index.md ~ patched 1 section' },
  ];

  const pages = [
    { path: 'modules/auth-middleware.md',     badge: 'new',     quiz: true },
    { path: 'concepts/jwt-token-lifecycle.md', badge: 'new',     quiz: true },
    { path: 'index.md',                        badge: 'updated', quiz: false },
  ];

  return (
    <div className="cb-app" data-screen-label={isRunning ? '02a Goal · Running' : '02b Goal · Completed'}>
      <Sidebar active="goals" />
      <main className="cb-main">
        <div className="cb-topbar" />
        <div className="cb-content cb-goal">
          <a className="cb-back-link" href="#">
            <span className="cb-back-arrow">←</span> Goals
          </a>

          <div className="cb-goal-head">
            <h1 className="cb-goal-title">{goalText}</h1>
            <div className="cb-goal-status">
              {isRunning ? (
                <>
                  <span className="cb-status-line">
                    <span className="cb-pulse-dot" />
                    <span className="cb-status-running">Running</span>
                    <span className="ter">·</span>
                    <span className="mono">23s</span>
                    <span className="ter">·</span>
                    <span className="mono">8.2k tokens</span>
                  </span>
                  <button className="cb-btn cb-btn-danger">Cancel</button>
                </>
              ) : (
                <>
                  <span className="cb-status-line">
                    <span>Completed in</span>
                    <span className="mono">47s</span>
                    <span className="ter">·</span>
                    <span className="mono">14.3k tokens</span>
                  </span>
                  <span className="cb-pill cb-pill-done">
                    <IconCheck size={11} /> Done
                  </span>
                </>
              )}
            </div>
          </div>

          {isRunning ? (
            <>
              <TimelineSection
                title="Reading codebase"
                count={3}
                rows={readingRows}
              />
              <TimelineSection
                title="Writing wiki"
                count={3}
                rows={writingRows}
                trailing={
                  <div className="cb-tl-row cb-tl-active">
                    <span className="cb-tl-icon" style={{ color: 'var(--accent)' }}><GSpinner /></span>
                    <span className="cb-tl-active-text">analyzing token validation flow…</span>
                    <span className="cb-tl-time mono">live</span>
                  </div>
                }
              />
              <StreamLog defaultOpen={false} lines={logLines} />
            </>
          ) : (
            <>
              <div className="cb-sec-head">
                <span className="cb-sec-label">Wiki pages changed</span>
                <span className="cb-sec-count">3</span>
              </div>
              <div className="cb-pages">
                {pages.map((p, i) => (
                  <div key={i} className="cb-page-row">
                    <span className="cb-tl-icon" style={{ color: 'var(--accent)' }}><GFileWrite /></span>
                    <span className="cb-tl-path mono">{p.path}</span>
                    <span className={'cb-mini-badge ' + p.badge}>{p.badge}</span>
                    <span className="cb-page-actions">
                      <button className="cb-btn cb-btn-sm">Open</button>
                      {p.quiz && <button className="cb-btn cb-btn-sm cb-btn-quiz">Quiz me</button>}
                    </span>
                  </div>
                ))}
              </div>
              <StreamLog defaultOpen={false} lines={logLines} />
            </>
          )}
        </div>
      </main>
    </div>
  );
}

window.GoalDetail = GoalDetail;
