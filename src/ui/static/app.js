// Multisbox Web UI — vanilla JS, no framework, no build step.

const $ = (id) => document.getElementById(id);
const $$ = (sel) => document.querySelectorAll(sel);

let config = null;
let dirty = false;

const PROFILE_OPTIONS = () => (config?.game_profiles || []).map(p =>
  `<option value="${escapeAttr(p.name)}">${escapeHtml(p.name)}</option>`).join('');
const ACCOUNT_OPTIONS = () => (config?.accounts || []).map(a =>
  `<option value="${escapeAttr(a.name)}">${escapeHtml(a.name)}</option>`).join('');
const REGION_OPTIONS = () => (config?.layout?.regions || []).map(r =>
  `<option value="${escapeAttr(r.name)}">${escapeHtml(r.name)}</option>`).join('');

function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}
function escapeAttr(s) { return escapeHtml(s); }

function markDirty() { dirty = true; $('dirty-indicator').classList.remove('hidden'); }

function switchTab(name) {
  $$('.tab-btn').forEach(b => b.classList.toggle('active', b.dataset.tab === name));
  $$('.tab').forEach(s => s.classList.toggle('active', s.id === 'tab-' + name));
}

async function loadConfig() {
  const r = await fetch('/api/config');
  config = await r.json();
  render();
  $('raw-json').value = JSON.stringify(config, null, 2);
}

async function save() {
  // If on raw tab, parse from textarea
  if ($$('.tab-btn.active')[0].dataset.tab === 'raw') {
    try {
      config = JSON.parse($('raw-json').value);
    } catch (e) {
      showStatus('JSON parse error: ' + e.message, 'err');
      return;
    }
  } else {
    // Collect from form
    config = collectFromForm();
  }
  const r = await fetch('/api/config', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(config)
  });
  const resp = await r.json();
  if (resp.ok) {
    showStatus('Saved.', 'ok');
    dirty = false;
    $('dirty-indicator').classList.add('hidden');
    loadConfig();
  } else {
    showStatus('Save failed: ' + resp.error, 'err');
  }
}

function showStatus(msg, type) {
  const el = $('status');
  el.textContent = msg;
  el.className = 'status ' + type;
  el.classList.remove('hidden');
  setTimeout(() => el.classList.add('hidden'), 5000);
}

function render() {
  $('team-name').value = config.team?.name || '';
  $('layout-name').value = config.layout?.name || '';

  // Slots
  const slots = $('slots-list');
  slots.innerHTML = (config.team?.slots || []).map((s, i) => slotItem(s, i)).join('');
  $$('#slots-list .item').forEach((el, i) => attachSlotHandlers(el, i));

  // Regions
  const regions = $('regions-list');
  regions.innerHTML = (config.layout?.regions || []).map((r, i) => regionItem(r, i)).join('');
  $$('#regions-list .item').forEach((el, i) => attachRegionHandlers(el, i));

  // Profiles
  const profiles = $('profiles-list');
  profiles.innerHTML = (config.game_profiles || []).map((p, i) => profileItem(p, i)).join('');
  $$('#profiles-list .item').forEach((el, i) => attachProfileHandlers(el, i));

  // Accounts
  const accounts = $('accounts-list');
  accounts.innerHTML = (config.accounts || []).map((a, i) => accountItem(a, i)).join('');
  $$('#accounts-list .item').forEach((el, i) => attachAccountHandlers(el, i));
}

function slotItem(s, i) {
  return `
    <div class="item" data-idx="${i}">
      <div class="item-header">
        <strong>Slot ${escapeHtml(s.index)}</strong>
        <button class="danger" data-action="del">Delete</button>
      </div>
      <div class="grid">
        <label>Index <input type="number" data-field="index" value="${s.index}"></label>
        <label>Account <select data-field="account">${ACCOUNT_OPTIONS().replace(`value="${s.account}"`, `value="${s.account}" selected`)}</select></label>
        <label>Region <select data-field="region">${REGION_OPTIONS().replace(`value="${s.region}"`, `value="${s.region}" selected`)}</select></label>
      </div>
    </div>`;
}

function attachSlotHandlers(el, i) {
  el.querySelectorAll('[data-field]').forEach(input => {
    input.addEventListener('change', () => {
      const field = input.dataset.field;
      let v = input.value;
      if (field === 'index') v = parseInt(v, 10);
      config.team.slots[i][field] = v;
      markDirty();
    });
  });
  el.querySelector('[data-action="del"]').addEventListener('click', () => {
    config.team.slots.splice(i, 1);
    markDirty();
    render();
  });
}

function regionItem(r, i) {
  return `
    <div class="item" data-idx="${i}">
      <div class="item-header">
        <strong>${escapeHtml(r.name)}</strong>
        <button class="danger" data-action="del">Delete</button>
      </div>
      <div class="grid">
        <label>Name <input type="text" data-field="name" value="${escapeAttr(r.name)}"></label>
        <label>X <input type="number" data-field="x" value="${r.x}"></label>
        <label>Y <input type="number" data-field="y" value="${r.y}"></label>
        <label>Width <input type="number" data-field="width" value="${r.width}"></label>
        <label>Height <input type="number" data-field="height" value="${r.height}"></label>
      </div>
    </div>`;
}

function attachRegionHandlers(el, i) {
  el.querySelectorAll('[data-field]').forEach(input => {
    input.addEventListener('change', () => {
      const field = input.dataset.field;
      let v = input.value;
      if (['x', 'y', 'width', 'height'].includes(field)) v = parseInt(v, 10);
      config.layout.regions[i][field] = v;
      markDirty();
    });
  });
  el.querySelector('[data-action="del"]').addEventListener('click', () => {
    config.layout.regions.splice(i, 1);
    markDirty();
    render();
  });
}

function profileItem(p, i) {
  const argsStr = (p.args || []).join(' ');
  return `
    <div class="item" data-idx="${i}">
      <div class="item-header">
        <strong>${escapeHtml(p.name)}</strong>
        <button class="danger" data-action="del">Delete</button>
      </div>
      <div class="grid">
        <label>Name <input type="text" data-field="name" value="${escapeAttr(p.name)}"></label>
        <label style="grid-column: 1 / -1;">Exe path <input type="text" data-field="exe_path" value="${escapeAttr(p.exe_path)}"></label>
        <label style="grid-column: 1 / -1;">Args (space-separated) <input type="text" data-field="args" value="${escapeAttr(argsStr)}"></label>
        <label style="grid-column: 1 / -1;">Working dir (optional) <input type="text" data-field="working_dir" value="${escapeAttr(p.working_dir || '')}"></label>
      </div>
    </div>`;
}

function attachProfileHandlers(el, i) {
  el.querySelectorAll('[data-field]').forEach(input => {
    input.addEventListener('change', () => {
      const field = input.dataset.field;
      let v = input.value;
      if (field === 'args') v = v.split(/\s+/).filter(Boolean);
      else if (field === 'working_dir' && v === '') v = null;
      config.game_profiles[i][field] = v;
      markDirty();
    });
  });
  el.querySelector('[data-action="del"]').addEventListener('click', () => {
    config.game_profiles.splice(i, 1);
    markDirty();
    render();
  });
}

function accountItem(a, i) {
  return `
    <div class="item" data-idx="${i}">
      <div class="item-header">
        <strong>${escapeHtml(a.name)}</strong>
        <button class="danger" data-action="del">Delete</button>
      </div>
      <div class="grid">
        <label>Name <input type="text" data-field="name" value="${escapeAttr(a.name)}"></label>
        <label>Game profile <select data-field="game_profile">${PROFILE_OPTIONS().replace(`value="${a.game_profile}"`, `value="${a.game_profile}" selected`)}</select></label>
      </div>
    </div>`;
}

function attachAccountHandlers(el, i) {
  el.querySelectorAll('[data-field]').forEach(input => {
    input.addEventListener('change', () => {
      const field = input.dataset.field;
      config.accounts[i][field] = input.value;
      markDirty();
    });
  });
  el.querySelector('[data-action="del"]').addEventListener('click', () => {
    config.accounts.splice(i, 1);
    markDirty();
    render();
  });
}

function collectFromForm() {
  const newCfg = JSON.parse(JSON.stringify(config));
  newCfg.team.name = $('team-name').value;
  newCfg.layout.name = $('layout-name').value;
  return newCfg;
}

// Tab switching
$$('.tab-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    if (btn.dataset.tab === 'raw') {
      $('raw-json').value = JSON.stringify(config, null, 2);
    }
    switchTab(btn.dataset.tab);
  });
});

// Add buttons
$('add-slot').addEventListener('click', () => {
  if (!config.team.slots) config.team.slots = [];
  const nextIdx = config.team.slots.length === 0
    ? 1
    : Math.max(...config.team.slots.map(s => s.index)) + 1;
  config.team.slots.push({
    index: nextIdx,
    account: config.accounts[0]?.name || '',
    region: config.layout.regions[0]?.name || ''
  });
  markDirty();
  render();
});

$('add-region').addEventListener('click', () => {
  if (!config.layout.regions) config.layout.regions = [];
  config.layout.regions.push({
    name: 'r' + (config.layout.regions.length + 1),
    x: 0, y: 0, width: 800, height: 600
  });
  markDirty();
  render();
});

$('add-profile').addEventListener('click', () => {
  if (!config.game_profiles) config.game_profiles = [];
  config.game_profiles.push({
    name: 'profile' + (config.game_profiles.length + 1),
    exe_path: '', args: [], working_dir: null
  });
  markDirty();
  render();
});

$('add-account').addEventListener('click', () => {
  if (!config.accounts) config.accounts = [];
  config.accounts.push({
    name: 'account' + (config.accounts.length + 1),
    game_profile: config.game_profiles[0]?.name || '',
    extra_args: null
  });
  markDirty();
  render();
});

// Save / Reload
$('save').addEventListener('click', save);
$('reload').addEventListener('click', loadConfig);

// Team/Layout name field handlers
['team-name', 'layout-name'].forEach(id => {
  $(id).addEventListener('change', markDirty);
});

// Boot
loadConfig().catch(e => showStatus('Load failed: ' + e.message, 'err'));
