/* global React */
// Shared sidebar across screens. Pass active=nav id and optionally
// override the back-link target.
function Sidebar({ active = 'goals', vault = { name: 'linear-clone', path: '~/code/linear-clone' } }) {
  const navItems = [
    { id: 'goals', emoji: '🚏', label: 'Goals', count: 12 },
    { id: 'wiki',  emoji: '📂', label: 'Wiki',  count: 38 },
    { id: 'quiz',  emoji: '🎓', label: 'Quiz',  count: 12 },
  ];
  return (
    <aside className="cb-sidebar">
      <div className="cb-back" title="Back to lobby">
        <span className="cb-back-arrow">←</span>
        <span>Lobby</span>
      </div>
      <div className="cb-vault">
        <div className="cb-vault-name">{vault.name}</div>
        <div className="cb-vault-path">{vault.path}</div>
      </div>
      <nav className="cb-nav">
        <div className="cb-nav-section">Vault</div>
        {navItems.map(n => (
          <div key={n.id} className={'cb-nav-item' + (active === n.id ? ' active' : '')}>
            <span className="cb-emoji">{n.emoji}</span>
            <span>{n.label}</span>
            <span className="cb-count">{n.count}</span>
          </div>
        ))}
      </nav>
      <div className="cb-sidebar-foot">
        <button className="cb-icon-btn" title="Settings"><IconSettings /></button>
        <button className="cb-icon-btn" title="Refresh index"><IconRefresh /></button>
        <div className="cb-kbd"><kbd>⌘</kbd><kbd>K</kbd></div>
      </div>
    </aside>
  );
}
window.Sidebar = Sidebar;
