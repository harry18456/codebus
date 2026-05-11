/* global React */
const { useState: useStateS } = React;

const SChev = () => (
  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="m6 9 6 6 6-6"/>
  </svg>
);
const SX = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18 6 6 18M6 6l12 12"/>
  </svg>
);

function Select({ value, suffix }) {
  return (
    <button className="cb-select" type="button">
      <span className="cb-select-value mono">{value}</span>
      {suffix && <span className="cb-select-suffix ter mono">{suffix}</span>}
      <span className="cb-select-chev"><SChev /></span>
    </button>
  );
}

function Slider({ min, max, value, format }) {
  const [v, setV] = useStateS(value);
  const pct = ((v - min) / (max - min)) * 100;
  return (
    <div className="cb-slider">
      <div className="cb-slider-track">
        <div className="cb-slider-fill" style={{ width: pct + '%' }} />
        <div className="cb-slider-thumb" style={{ left: pct + '%' }} />
      </div>
      <input
        className="cb-slider-input"
        type="range" min={min} max={max} value={v}
        onChange={e => setV(Number(e.target.value))}
      />
      <span className="cb-slider-readout mono">{format ? format(v) : v}</span>
      <span className="cb-slider-range ter mono">{min}–{max}{format && format(max).includes('%') ? '%' : ''}</span>
    </div>
  );
}

function Settings() {
  return (
    <div className="cb-cmdk-root" data-screen-label="06 Settings · Modal">
      <div className="cb-cmdk-backdrop" aria-hidden="true">
        <VaultWorkspace />
      </div>
      <div className="cb-cmdk-scrim" style={{ background: 'rgba(0,0,0,.55)', backdropFilter: 'none' }} />

      <div className="cb-modal-card" role="dialog" aria-label="Global Settings">
        <header className="cb-modal-head">
          <h2 className="cb-modal-title">Global Settings</h2>
          <button className="cb-icon-btn cb-modal-x" aria-label="Close"><SX /></button>
        </header>

        <div className="cb-modal-body">
          <div className="cb-form">

            <div className="cb-form-row">
              <div className="cb-form-label">AI Provider</div>
              <div className="cb-form-ctl">
                <span className="cb-form-text">Claude CLI</span>
                <span className="cb-form-aux ter">only option for now</span>
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">Authentication</div>
              <div className="cb-form-ctl">
                <span className="cb-pill cb-pill-done"><IconCheck size={11} /> Connected</span>
                <button className="cb-link-btn">Re-authenticate…</button>
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">
                Default model
                <div className="cb-form-help ter">applies to all runs</div>
              </div>
              <div className="cb-form-ctl cb-form-stack">
                <div className="cb-form-subrow">
                  <span className="cb-form-sublabel mono">goal</span>
                  <Select value="sonnet" />
                </div>
                <div className="cb-form-subrow">
                  <span className="cb-form-sublabel mono">query</span>
                  <Select value="haiku" />
                </div>
                <div className="cb-form-subrow">
                  <span className="cb-form-sublabel mono">fix</span>
                  <Select value="sonnet" />
                </div>
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">PII scanner</div>
              <div className="cb-form-ctl">
                <Select value="regex_basic" suffix="· 14 patterns" />
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">Log sink</div>
              <div className="cb-form-ctl">
                <span className="cb-path mono">~/.codebus/logs/</span>
                <button className="cb-link-btn">Change folder…</button>
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">
                Quiz pass threshold
                <div className="cb-form-help ter">% correct to pass a quiz attempt</div>
              </div>
              <div className="cb-form-ctl">
                <Slider min={50} max={100} value={80} format={v => v + '%'} />
              </div>
            </div>

            <div className="cb-form-row">
              <div className="cb-form-label">Default quiz length</div>
              <div className="cb-form-ctl">
                <Slider min={3} max={10} value={5} format={v => v + ' questions'} />
              </div>
            </div>

          </div>
        </div>

        <footer className="cb-modal-foot">
          <span className="cb-modal-foot-note ter">
            Reads/writes <span className="mono">~/.codebus/config.yaml</span>
          </span>
          <div className="cb-modal-foot-actions">
            <button className="cb-btn">
              Cancel <span className="cb-kshort">ESC</span>
            </button>
            <button className="cb-btn primary">
              Save <span className="cb-kshort">⌘S</span>
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

window.Settings = Settings;
