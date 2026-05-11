/* global React */
function Lobby({ state = 'populated' }) {
  const empty = state === 'empty';

  const vaults = [
    { name: 'linear-clone',   path: '~/code/linear-clone',   age: '2h ago',   pinned: true },
    { name: 'fern-platform',  path: '~/work/fern-platform',  age: 'yesterday' },
    { name: 'codebus',        path: '~/code/codebus',        age: '3d ago' },
  ];

  return (
    <div className="cb-lobby-app" data-screen-label={empty ? '04b Lobby · Empty' : '04a Lobby · Populated'}>
      <div className="cb-topbar cb-lobby-topbar">
        <div className="cb-lobby-brand">
          <span className="cb-lobby-emoji">🚌</span>
          <span className="cb-lobby-wordmark">codebus</span>
        </div>
        {!empty && (
          <button className="cb-btn primary">
            <IconPlus /> New Vault <span className="cb-kshort">⌘N</span>
          </button>
        )}
      </div>

      {empty ? (
        <div className="cb-lobby-content cb-lobby-empty">
          <div className="cb-empty">
            <div className="cb-empty-emoji">🚌</div>
            <h1 className="cb-empty-title">來搭第一台公車吧</h1>
            <p className="cb-empty-sub">
              codebus 把 LLM 探索程式碼的中間態持久化成你的旅遊書。
            </p>
            <button className="cb-btn primary cb-empty-cta">
              <IconPlus /> Board a new bus
            </button>

            <div className="cb-quickstart">
              <div className="cb-quickstart-head">
                <span className="cb-sec-label">Quick start</span>
              </div>
              <ol className="cb-quickstart-steps">
                <li>
                  <span className="cb-qs-num mono">1</span>
                  <span className="cb-qs-text">Pick a repo folder</span>
                </li>
                <li>
                  <span className="cb-qs-num mono">2</span>
                  <span className="cb-qs-text">
                    Run a goal:{' '}
                    <span className="cb-qs-quote mono">搞懂這 repo 的 X</span>
                  </span>
                </li>
                <li>
                  <span className="cb-qs-num mono">3</span>
                  <span className="cb-qs-text">Quiz yourself to verify</span>
                </li>
              </ol>
            </div>
          </div>
        </div>
      ) : (
        <div className="cb-lobby-content">
          <div className="cb-lobby-inner">
            <div className="cb-sec-head" style={{ marginTop: 0 }}>
              <span className="cb-sec-label">Recent vaults</span>
              <span className="cb-sec-count">{vaults.length}</span>
            </div>

            <div className="cb-vault-list">
              {vaults.map((v, i) => (
                <div key={i} className="cb-vault-card">
                  <div className="cb-vault-card-main">
                    <div className="cb-vault-card-row">
                      <span className="cb-vault-card-name">{v.name}</span>
                      <span className="cb-vault-card-path mono">{v.path}</span>
                    </div>
                    <div className="cb-vault-card-meta">
                      <span>last opened</span>
                      <span className="mono cb-vault-card-age">{v.age}</span>
                    </div>
                  </div>
                  <button className="cb-icon-btn cb-vault-card-kebab"><IconKebab /></button>
                </div>
              ))}
            </div>

            <div className="cb-lobby-hint">
              <span className="ter">tip ·</span>
              <span>Drag a repo folder anywhere into this window to open it as a vault.</span>
            </div>
          </div>
        </div>
      )}

      <div className="cb-lobby-foot">
        <button className="cb-foot-link">
          <IconSettings size={12} /> Settings
        </button>
        <span className="cb-foot-version mono">v0.1.0</span>
      </div>
    </div>
  );
}
window.Lobby = Lobby;
