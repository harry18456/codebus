/* global React */
const { useEffect: useEffectCK, useState: useStateCK } = React;

function CmdKOverlay({ state = 'streaming' }) {
  // 'streaming'  — answer mid-stream (caret visible after partial text)
  // 'answered'   — full answer settled, cited chips visible
  // 'idle'       — overlay just opened, empty response area
  const idle = state === 'idle';
  const streaming = state === 'streaming';

  const question = 'How does the auth middleware decide whether to challenge?';

  // A realistic LLM answer paragraph. Streaming state truncates near the end.
  const fullAnswer = (
    <>
      The middleware reads the <code className="cb-code-i">Authorization</code> header on every
      incoming request. If a bearer token is present, it’s validated via{' '}
      <code className="cb-code-i">verifyJwt()</code>: on success the decoded user is attached
      to <code className="cb-code-i">req.user</code> and the chain continues; on failure the
      request is rejected with <span className="mono cb-inline-kbd">401</span> before any route
      handler runs. If the header is absent, the request continues unauthenticated and the
      route-level guards decide whether to challenge with{' '}
      <code className="cb-code-i">redirectToLogin()</code>.
    </>
  );
  const streamAnswer = (
    <>
      The middleware reads the <code className="cb-code-i">Authorization</code> header on every
      incoming request. If a bearer token is present, it’s validated via{' '}
      <code className="cb-code-i">verifyJwt()</code>: on success the decoded user is attached
      to <code className="cb-code-i">req.user</code> and the chain continues; on failure the
      request is rejected with <span className="mono cb-inline-kbd">401</span>
    </>
  );

  return (
    <div className="cb-cmdk-root" data-screen-label="05 Cmd+K Overlay">
      {/* blurred workspace backdrop */}
      <div className="cb-cmdk-backdrop" aria-hidden="true">
        <VaultWorkspace />
      </div>
      <div className="cb-cmdk-scrim" aria-hidden="true" />

      {/* card */}
      <div className="cb-cmdk-card" role="dialog" aria-label="Ask the wiki">
        {!idle && (
          <div className="cb-cmdk-response">
            <div className="cb-cmdk-question">
              <span className="cb-cmdk-q-tag mono">you asked</span>
              <span className="cb-cmdk-q-text">{question}</span>
            </div>

            <div className="cb-cmdk-answer">
              {streaming ? streamAnswer : fullAnswer}
              {streaming && <span className="cb-cmdk-caret" />}
            </div>

            {!streaming && (
              <div className="cb-cmdk-cited">
                <div className="cb-cited-head">
                  <span className="cb-cited-arrow mono">▾</span>
                  <span className="cb-sec-label" style={{ fontSize: 10 }}>Cited</span>
                  <span className="cb-sec-count mono">2</span>
                </div>
                <div className="cb-cited-chips">
                  <a className="cb-cited-chip" href="#">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z"/><path d="M14 3v5h5"/>
                    </svg>
                    <span className="mono">modules/auth-middleware.md</span>
                  </a>
                  <a className="cb-cited-chip" href="#">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z"/><path d="M14 3v5h5"/>
                    </svg>
                    <span className="mono">concepts/jwt-token-lifecycle.md</span>
                  </a>
                </div>
              </div>
            )}
          </div>
        )}

        {idle && (
          <div className="cb-cmdk-idle">
            <div className="cb-cmdk-idle-text">
              Ask anything about <span className="mono">linear-clone</span>. Answers cite the wiki pages they’re drawn from.
            </div>
          </div>
        )}

        {/* divider */}
        <div className="cb-cmdk-div" />

        {/* input bar */}
        <div className="cb-cmdk-input">
          <span className="cb-cmdk-prompt mono">›</span>
          <span className="cb-cmdk-field" data-empty={idle}>
            {idle
              ? <span className="cb-cmdk-placeholder">你想知道什麼？  ·  Ask anything…</span>
              : <span className="cb-cmdk-query">{question}</span>
            }
          </span>
          <span className="cb-cmdk-enter mono">⏎</span>
        </div>
      </div>

      {/* foot hints */}
      <div className="cb-cmdk-foot">
        <span className="cb-cmdk-foothint"><kbd className="cb-kbd-inline">↑↓</kbd> nav cited</span>
        <span className="cb-cmdk-foothint"><kbd className="cb-kbd-inline">⌘ ⏎</kbd> open citation</span>
        <span className="cb-cmdk-foothint cb-cmdk-esc"><kbd className="cb-kbd-inline">ESC</kbd> to close</span>
      </div>
    </div>
  );
}
window.CmdKOverlay = CmdKOverlay;
