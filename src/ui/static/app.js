// Multisbox Web UI — vanilla JS, no framework, no build step.

const $ = (id) => document.getElementById(id);
const $$ = (sel) => document.querySelectorAll(sel);

let config = null;
let dirty = false;
let previewCanvas = null;
let previewCtx = null;
let dragging = null;
let dragOffset = { x: 0, y: 0 };

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

// --- Wizard logic ---
let wizardStep = 1;
let wizardGame = 'gw2';
let wizardAccountCount = 4;
let wizardLayout = 'grid2x2';

function showWizardStep(step) {
  $$('.wizard-step').forEach(s => s.classList.add('hidden'));
  $(`.wizard-step[data-step="${step}"]`)?.classList?.remove('hidden');
  wizardStep = step;
}

function showWizard() {
  $('wizard').classList.remove('hidden');
  showWizardStep(1);
}

function hideWizard() {
  $('wizard').classList.add('hidden');
}

function handleWizardNext() {
  if (wizardStep === 1) {
    showWizardStep(2);
  } else if (wizardStep === 2) {
    wizardGame = $$('input[name="game"]:checked')[0]?.value || 'gw2';
    showWizardStep(3);
  } else if (wizardStep === 3) {
    wizardAccountCount = parseInt($('account-count')?.value || '4', 10);
    wizardLayout = $('layout-type')?.value || 'grid2x2';
    createWizardConfig();
    showWizardStep(4);
  }
}

function handleWizardPrev() {
  if (wizardStep > 1) showWizardStep(wizardStep - 1);
}

async function createWizardConfig() {
  const body = {
    game: wizardGame,
    accountCount: wizardAccountCount,
    layout: wizardLayout
  };
  const r = await fetch('/api/wizard/create', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  });
  const resp = await r.json();
  if (resp.ok) {
    config = resp.config;
    render();
    $('raw-json').value = JSON.stringify(config, null, 2);
    showStatus('Config created from wizard!', 'ok');
  } else {
    showStatus('Failed: ' + resp.error, 'err');
  }
}

function handleCustomGameToggle() {
  const isCustom = $$('input[name="game"]:checked')[0]?.value === 'custom';
  $('custom-game-fields')?.classList?.toggle('hidden', !isCustom);
}

function handleWizardFinish() {
  hideWizard();
  switchTab('team');
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
loadConfig().then(() => {
  // Show wizard if config is minimal/empty
  const hasRealConfig = config?.game_profiles?.length > 0 &&
    config?.accounts?.length > 0 &&
    config?.team?.slots?.length > 0;
  if (!hasRealConfig) {
    showWizard();
  }
}).catch(e => showStatus('Load failed: ' + e.message, 'err'));

// Wizard event listeners
document.addEventListener('click', e => {
  if (e.target.matches('[data-next]')) handleWizardNext();
  if (e.target.matches('[data-prev]')) handleWizardPrev();
  if (e.target.id === 'wizard-create') handleWizardNext();
  if (e.target.id === 'wizard-finish') handleWizardFinish();
});

document.addEventListener('change', e => {
  if (e.target.matches('input[name="game"]')) handleCustomGameToggle();
});

// --- Layout Preview ---
function initPreview() {
  previewCanvas = $('layout-preview');
  if (!previewCanvas) return;
  previewCtx = previewCanvas.getContext('2d');
  previewCanvas.addEventListener('mousedown', onPreviewMouseDown);
  previewCanvas.addEventListener('mousemove', onPreviewMouseMove);
  previewCanvas.addEventListener('mouseup', onPreviewMouseUp);
  previewCanvas.addEventListener('mouseleave', onPreviewMouseUp);
  renderPreview();
}

function renderPreview() {
  if (!previewCtx || !config?.layout?.regions) return;
  const canvas = previewCanvas;
  const ctx = previewCtx;
  const regions = config.layout.regions;

  // Find bounding box of all regions
  let maxX = 1920, maxY = 1080;
  regions.forEach(r => {
    maxX = Math.max(maxX, r.x + r.width);
    maxY = Math.max(maxY, r.y + r.height);
  });

  // Scale to fit canvas
  const scaleX = canvas.width / maxX;
  const scaleY = canvas.height / maxY;
  const scale = Math.min(scaleX, scaleY) * 0.9;
  const offsetX = (canvas.width - maxX * scale) / 2;
  const offsetY = (canvas.height - maxY * scale) / 2;

  ctx.clearRect(0, 0, canvas.width, canvas.height);

  // Draw background grid
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 1;
  for (let x = 0; x <= maxX; x += 100) {
    ctx.beginPath();
    ctx.moveTo(offsetX + x * scale, offsetY);
    ctx.lineTo(offsetX + x * scale, offsetY + maxY * scale);
    ctx.stroke();
  }
  for (let y = 0; y <= maxY; y += 100) {
    ctx.beginPath();
    ctx.moveTo(offsetX, offsetY + y * scale);
    ctx.lineTo(offsetX + maxX * scale, offsetY + y * scale);
    ctx.stroke();
  }

  // Draw regions
  const colors = ['#4a9eff', '#ff6b6b', '#51cf66', '#ffd43b', '#cc5de8', '#ff922b'];
  regions.forEach((r, i) => {
    const x = offsetX + r.x * scale;
    const y = offsetY + r.y * scale;
    const w = r.width * scale;
    const h = r.height * scale;

    ctx.fillStyle = colors[i % colors.length] + '40';
    ctx.fillRect(x, y, w, h);

    ctx.strokeStyle = colors[i % colors.length];
    ctx.lineWidth = 2;
    ctx.strokeRect(x, y, w, h);

    // Label
    ctx.fillStyle = '#fff';
    ctx.font = '12px monospace';
    ctx.textAlign = 'center';
    ctx.fillText(r.name, x + w / 2, y + h / 2 - 8);
    ctx.font = '10px monospace';
    ctx.fillText(`${r.width}x${r.height}`, x + w / 2, y + h / 2 + 8);

    // Store scaled coords for drag detection
    r._scaled = { x, y, w, h, scale, offsetX, offsetY };
  });
}

function onPreviewMouseDown(e) {
  if (!config?.layout?.regions) return;
  const rect = previewCanvas.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;

  // Find which region was clicked (reverse order for z-order)
  for (let i = config.layout.regions.length - 1; i >= 0; i--) {
    const r = config.layout.regions[i];
    if (!r._scaled) continue;
    const s = r._scaled;
    if (mx >= s.x && mx <= s.x + s.w && my >= s.y && my <= s.y + s.h) {
      dragging = { index: i, startX: mx, startY: my, origX: r.x, origY: r.y };
      dragOffset.x = mx - s.x;
      dragOffset.y = my - s.y;
      break;
    }
  }
}

function onPreviewMouseMove(e) {
  if (!dragging) return;
  const rect = previewCanvas.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  const r = config.layout.regions[dragging.index];
  const scale = r._scaled.scale;

  // Calculate new position
  const newX = Math.round((mx - dragOffset.x - r._scaled.offsetX) / scale);
  const newY = Math.round((my - dragOffset.y - r._scaled.offsetY) / scale);

  // Snap to grid (10px)
  r.x = Math.round(newX / 10) * 10;
  r.y = Math.round(newY / 10) * 10;

  markDirty();
  renderPreview();
  render(); // Update form fields
}

function onPreviewMouseUp() {
  dragging = null;
}

// Named layouts
function renderNamedLayouts() {
  const container = $('named-layouts-list');
  if (!container || !config?.named_layouts) return;

  container.innerHTML = config.named_layouts.map((l, i) => `
    <div class="item" data-idx="${i}">
      <div class="item-header">
        <strong>${escapeHtml(l.name)}</strong>
        <div>
          <button data-action="load">Load</button>
          <button class="danger" data-action="del">Delete</button>
        </div>
      </div>
      <div class="grid">
        <label>Name <input type="text" data-field="name" value="${escapeAttr(l.name)}"></label>
        <label>Regions: ${l.regions.length}</label>
      </div>
    </div>
  `).join('');

  container.querySelectorAll('[data-action="load"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const idx = parseInt(btn.closest('.item').dataset.idx);
      loadNamedLayout(idx);
    });
  });

  container.querySelectorAll('[data-action="del"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const idx = parseInt(btn.closest('.item').dataset.idx);
      config.named_layouts.splice(idx, 1);
      markDirty();
      renderNamedLayouts();
    });
  });
}

function loadNamedLayout(idx) {
  const layout = config.named_layouts[idx];
  if (!layout) return;
  config.layout = JSON.parse(JSON.stringify(layout));
  markDirty();
  render();
  renderPreview();
  showStatus(`Loaded layout: ${layout.name}`, 'ok');
}

$('save-layout')?.addEventListener('click', () => {
  if (!config.named_layouts) config.named_layouts = [];
  const name = prompt('Layout name:');
  if (!name) return;
  config.named_layouts.push(JSON.parse(JSON.stringify(config.layout)));
  config.named_layouts[config.named_layouts.length - 1].name = name;
  markDirty();
  renderNamedLayouts();
  showStatus(`Saved layout: ${name}`, 'ok');
});

// Initialize preview on load
document.addEventListener('DOMContentLoaded', () => {
  setTimeout(initPreview, 100);
});
