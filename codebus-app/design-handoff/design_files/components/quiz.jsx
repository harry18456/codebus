/* global React */
const { useEffect: useEffectQ, useState: useStateQ } = React;

function Quiz({ state = 'pending' }) {
  const reviewing = state === 'review';

  const choices = [
    { id: 'a', label: 'In the controller' },
    { id: 'b', label: 'In the middleware' },
    { id: 'c', label: 'In the database layer' },
    { id: 'd', label: 'In the frontend' },
  ];
  const correct = 'b';
  // pending: user has tentatively picked B; review: user submitted A (wrong)
  const initialPick = reviewing ? 'a' : 'b';
  const [picked, setPicked] = useStateQ(initialPick);
  const [pressed, setPressed] = useStateQ(null); // visual flash on Enter / →

  // Standard quiz keybindings: A–D select (pending only), Enter submits,
  // → advances. In a locked-state mock the latter two just flash the
  // button so the binding is visible; the real app would actually advance.
  useEffectQ(() => {
    const onKey = (e) => {
      const k = e.key.toLowerCase();
      if (!reviewing && ['a','b','c','d'].includes(k)) {
        setPicked(k); e.preventDefault();
      } else if (e.key === 'Enter' && !reviewing) {
        setPressed('submit'); setTimeout(() => setPressed(null), 180);
      } else if (e.key === 'ArrowRight' && reviewing) {
        setPressed('next');   setTimeout(() => setPressed(null), 180);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [reviewing]);

  const choiceState = (id) => {
    if (reviewing) {
      if (id === correct) return 'correct';
      if (id === 'a')     return 'wrong';      // user's locked wrong pick
      return 'dim';
    }
    return id === picked ? 'selected' : 'idle';
  };

  return (
    <div className="cb-app" data-screen-label={reviewing ? '03b Quiz · Reviewing' : '03a Quiz · Pending'}>
      <Sidebar active="quiz" />
      <main className="cb-main">
        <div className="cb-topbar" />
        <div className="cb-content cb-quiz-wrap">
          <div className="cb-quiz">

            <div className="cb-quiz-head">
              <div className="cb-quiz-meta">
                <span className="ter">Quiz:</span>
                <span className="mono cb-quiz-page">auth-middleware</span>
              </div>
              <div className="cb-quiz-counter">
                <span className="mono">Q3</span>
                <span className="ter">of</span>
                <span className="mono">5</span>
              </div>
            </div>

            <h1 className="cb-quiz-q">
              <span className="cb-quiz-q-num mono">Q3.</span>
              Where does authentication start?
            </h1>

            <div className="cb-quiz-choices">
              {choices.map(c => {
                const s = choiceState(c.id);
                return (
                  <div key={c.id}
                       className={'cb-choice cb-choice-' + s}
                       role="button" tabIndex={0}
                       onClick={() => !reviewing && setPicked(c.id)}>
                    <span className="cb-choice-key mono">{c.id.toUpperCase()}</span>
                    <span className="cb-choice-radio">
                      {s === 'selected' && <span className="cb-radio-dot" />}
                      {s === 'correct'  && <IconCheck size={12} />}
                      {s === 'wrong'    && <span className="cb-x">✕</span>}
                    </span>
                    <span className="cb-choice-label">{c.label}</span>
                    {s === 'wrong'   && <span className="cb-choice-tag wrong">your answer</span>}
                    {s === 'correct' && <span className="cb-choice-tag correct">correct</span>}
                  </div>
                );
              })}
            </div>

            {reviewing && (
              <blockquote className="cb-quote">
                <span className="cb-quote-mark">“</span>
                Auth middleware runs before route handlers per
                {' '}<a className="cb-wikilink mono" href="#">[[auth-flow#middleware]]</a>.
              </blockquote>
            )}

            <div className="cb-quiz-actions">
              {reviewing ? (
                <>
                  <a className="cb-back-link" href="#">
                    <span className="cb-back-arrow">←</span> Back to wiki page
                  </a>
                  <button className={'cb-btn primary cb-quiz-next' + (pressed === 'next' ? ' cb-flash' : '')}>
                    Next: Q4 <span className="cb-arrow-r">→</span>
                  </button>
                </>
              ) : (
                <>
                  <span className="cb-quiz-hint ter">
                    <kbd className="cb-kbd-inline">⏎</kbd> to submit
                  </span>
                  <button className={'cb-btn primary cb-quiz-submit' + (pressed === 'submit' ? ' cb-flash' : '')}>Submit</button>
                </>
              )}
            </div>

          </div>
        </div>
      </main>
    </div>
  );
}
window.Quiz = Quiz;
