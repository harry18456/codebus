/* Reusable HTML strings for shell pieces. Use innerHTML/include. */

window.CB_TOPBAR = ({ workspace = 'demo-synthetic', task = '', tab = null, kill = 'READY' } = {}) => `
<div class="cb-topbar">
  <div class="cb-brand">
    <div class="logo">🚌</div>
    <span>CodeBus</span>
  </div>
  <button class="cb-ws-switch" title="Switch workspace">
    <svg class="cb-ico" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M2 4a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V4Z"/></svg>
    <span>${workspace}</span>
    <span class="caret">▾</span>
  </button>
  ${tab ? `
  <div class="cb-tabs">
    <button class="${tab==='learn'?'active':''}">Learn</button>
    <button class="${tab==='reasoning'?'active':''}">Reasoning</button>
    <button class="${tab==='audit'?'active':''}">Audit</button>
  </div>` : ''}
  <div class="cb-spacer"></div>
  ${task ? `<div style="font-family:var(--mono);font-size:10.5px;color:var(--text-mute);padding-right:6px;border-right:1px solid var(--border);margin-right:6px">task=${task}</div>` : ''}
  <div class="cb-session">
    <div class="seg" title="LLM provider"><span class="dot accent"></span>gpt-4o-mini</div>
    <div class="seg" title="Tokens this session">tokens <span class="val">12.4K</span></div>
    <div class="seg" title="Cost">cost <span class="val">$0.18</span></div>
  </div>
  <button class="cb-ws-switch" title="Settings" style="padding:5px 8px">
    <svg class="cb-ico" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="2.5"/><path d="M8 1v2M8 13v2M15 8h-2M3 8H1M12.95 3.05l-1.41 1.41M4.46 11.54l-1.41 1.41M12.95 12.95l-1.41-1.41M4.46 4.46L3.05 3.05"/></svg>
  </button>
  <div class="cb-kill" title="Kill switch — instantly disable LLM calls">
    <div class="pulse"></div>
    <span>${kill}</span>
  </div>
</div>
`;

/* audit rail with sample data */
window.CB_AUDIT_RAIL = ({ activeTab = 'sanitize', counts = {}, fresh = null } = {}) => {
  const c = Object.assign({
    sanitize: 12, tool: 47, reasoning: 28, token: 28, llm: 14, kb_growth: 0, generator: 0
  }, counts);
  const tab = (key, label, n) => `
    <button class="${activeTab===key?'active':''}" data-tab="${key}">${label}<span class="count">${n}</span></button>`;
  return `
<div class="cb-audit">
  <div class="cb-audit-head">
    <div class="cb-audit-title">
      <span>workspace audit · &lt;ws&gt;/.codebus/</span>
      <span class="live"><span class="dot"></span>LIVE</span>
    </div>
    <div class="cb-audit-tabs">
      ${tab('sanitize', 'sanitize', c.sanitize)}
      ${tab('tool', 'tool', c.tool)}
      ${tab('reasoning', 'reason', c.reasoning)}
      ${tab('token', 'token', c.token)}
      ${tab('llm', 'llm', c.llm)}
      ${tab('kb_growth', 'kb_growth', c.kb_growth)}
      ${tab('generator', 'generator', c.generator)}
    </div>
  </div>
  <div class="cb-audit-body" id="cb-audit-body"></div>
  <div class="cb-audit-foot">
    <div class="row"><span>tokens</span><span class="v">12,432<span class="delta">+1.2K</span></span></div>
    <div class="row"><span>cost</span><span class="v">$0.18<span class="delta">+$0.04</span></span></div>
    <div class="row"><span>chunks</span><span class="v">147</span></div>
    <div class="row"><span>sanitize hits</span><span class="v">12</span></div>
  </div>
</div>
`;
};

/* sample data for audit rows by tab. Data is shaped from real demo-synthetic style. */
window.CB_AUDIT_SAMPLES = {
  sanitize: [
    { ts: '14:23:08', kind: 'secret', file: 'src/config.py', line: 12, badge: 'pass1', badgeCls: 'purple' },
    { ts: '14:23:09', kind: 'email',  file: 'src/auth/user.py', line: 47, badge: 'pass1', badgeCls: 'purple' },
    { ts: '14:23:14', kind: 'secret', file: 'tests/fixtures/.env.test', line: 3, badge: 'pass1', badgeCls: 'purple' },
    { ts: '14:23:21', kind: 'pii_id', file: 'docs/example.md', line: 22, badge: 'pass2', badgeCls: 'purple' },
  ],
  tool: [
    { ts: '14:24:02', body: 'list_dir(<span class="key">/src</span>)', badge: 'allow', badgeCls: 'green' },
    { ts: '14:24:04', body: 'read_file(<span class="key">/src/storage/local.ts</span>)', badge: 'allow', badgeCls: 'green' },
    { ts: '14:24:06', body: 'search(<span class="key">"interface Storage"</span>)', badge: 'kb', badgeCls: 'accent' },
    { ts: '14:24:09', body: 'trace_import(<span class="key">MockStorageAdapter</span>)', badge: 'allow', badgeCls: 'green' },
  ],
  reasoning: [
    { ts: '14:24:01', body: '<span class="key">Think</span> · 我先看 storage/ 結構…', badge: 'step #01', badgeCls: '' },
    { ts: '14:24:04', body: '<span class="key">Act</span> · list_dir(/src/storage)', badge: 'step #02', badgeCls: '' },
    { ts: '14:24:07', body: '<span class="key">Judge</span> · 找到 adapter 介面，relevance=0.8', badge: 'step #03', badgeCls: 'green' },
    { ts: '14:24:11', body: '<span class="key">Coverage</span> · gap: 找不到 prod adapter', badge: 'step #04', badgeCls: 'yellow' },
  ],
  token: [
    { ts: '14:24:01', body: 'module=<span class="key">reasoning</span> in=512 out=128', badge: '$0.001', badgeCls: '' },
    { ts: '14:24:08', body: 'module=<span class="key">judge</span> in=380 out=64', badge: '$0.0006', badgeCls: '' },
    { ts: '14:24:14', body: 'module=<span class="key">coverage</span> in=290 out=42', badge: '$0.0004', badgeCls: '' },
    { ts: '14:24:21', body: 'module=<span class="key">reasoning</span> in=720 out=180', badge: '$0.001', badgeCls: '' },
  ],
  llm: [
    { ts: '14:24:01', body: 'POST chat · sanitizer_pass2_applied=<span class="key">true</span>', badge: '<span class="placeholder">&lt;REDACTED:secret#1&gt;</span>', badgeCls: '' },
    { ts: '14:24:08', body: 'POST chat · pass2=true · 4 placeholders', badge: 'wire ↗', badgeCls: 'accent' },
  ],
  kb_growth: [
    { ts: '14:32:14', body: 'add · originating=<span class="key">s02-storage</span>', badge: '+1 chunk', badgeCls: 'green' },
  ],
  generator: [
    { ts: '14:30:02', body: 'station <span class="key">s01-types</span> · 1/5 ok', badge: 'ok', badgeCls: 'green' },
    { ts: '14:30:14', body: 'station <span class="key">s02-storage</span> · 2/5 retry=1', badge: 'retry', badgeCls: 'yellow' },
    { ts: '14:30:31', body: 'station <span class="key">s02-storage</span> · 2/5 ok', badge: 'ok', badgeCls: 'green' },
  ],
};

window.CB_renderAuditRows = (tab, freshIndex = -1) => {
  const rows = (window.CB_AUDIT_SAMPLES[tab] || []);
  return rows.map((r, i) => {
    const main = r.body || `<span class="key">${r.kind}</span> in <span style="opacity:.85">${r.file}:${r.line}</span>`;
    return `
    <div class="cb-audit-row ${i===freshIndex?'fresh':''}">
      <div class="ts">${r.ts}</div>
      <div class="body">${main}</div>
      <div class="badge ${r.badgeCls||''}">${r.badge||''}</div>
    </div>`;
  }).join('');
};

window.CB_mountAudit = (rootSel, opts = {}) => {
  const root = document.querySelector(rootSel);
  if (!root) return;
  let active = opts.activeTab || 'sanitize';
  root.innerHTML = window.CB_AUDIT_RAIL({ activeTab: active, counts: opts.counts });
  const bodyEl = root.querySelector('#cb-audit-body');
  const tabsEl = root.querySelector('.cb-audit-tabs');
  const render = () => { bodyEl.innerHTML = window.CB_renderAuditRows(active, opts.fresh ?? -1); };
  render();
  tabsEl.addEventListener('click', (e) => {
    const btn = e.target.closest('button[data-tab]');
    if (!btn) return;
    active = btn.dataset.tab;
    tabsEl.querySelectorAll('button').forEach(b => b.classList.toggle('active', b.dataset.tab === active));
    render();
  });
};
