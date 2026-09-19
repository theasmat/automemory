// AutoMemory Pro Dashboard JavaScript

let currentTab = 'overview';

document.addEventListener('DOMContentLoaded', () => {
  setupTabs();
  loadStats();
  loadRecentEvents();
  loadMemories();
  loadTraces();
  loadFailures();
  loadProjects();
  loadPreferences();

  // Poll stats and events every 4 seconds
  setInterval(() => {
    loadStats();
    if (currentTab === 'overview') {
      loadRecentEvents();
    }
  }, 4000);

  // Search input handler
  const memSearch = document.getElementById('memory-search-input');
  if (memSearch) {
    memSearch.addEventListener('keyup', (e) => {
      if (e.key === 'Enter') {
        loadMemories(memSearch.value.trim());
      }
    });
  }

  const recallBtn = document.getElementById('btn-test-recall');
  if (recallBtn) {
    recallBtn.addEventListener('click', runRecallTest);
  }
});

function setupTabs() {
  const tabs = document.querySelectorAll('.nav-tab');
  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));

      tab.classList.add('active');
      currentTab = tab.dataset.tab;
      const target = document.getElementById(`tab-${currentTab}`);
      if (target) {
        target.classList.add('active');
      }
    });
  });
}

async function loadStats() {
  try {
    const res = await fetch('/api/stats');
    if (!res.ok) return;
    const data = await res.json();
    document.getElementById('stat-active-memories').innerText = data.active_memories || 0;
    document.getElementById('stat-traces').innerText = data.active_traces || 0;
    document.getElementById('stat-failures').innerText = data.unresolved_failures || 0;
    document.getElementById('stat-projects').innerText = data.total_projects || 0;
  } catch (e) {
    console.error('Stats error:', e);
  }
}

async function loadRecentEvents() {
  const container = document.getElementById('event-list');
  if (!container) return;
  try {
    const res = await fetch('/api/events');
    if (!res.ok) return;
    const events = await res.json();
    if (events.length === 0) {
      container.innerHTML = '<div class="empty-state">No events captured yet. Run an agent turn to see events!</div>';
      return;
    }
    container.innerHTML = events.map(e => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${e.agent.adapter_id}</span> ${e.event_type}</span>
          <span>${new Date(e.timestamp).toLocaleTimeString()}</span>
        </div>
        <div class="item-title">${e.tool ? e.tool.name : (e.task_id || e.session_id)}</div>
        <div class="item-content">${e.tool && e.tool.input ? JSON.stringify(e.tool.input).slice(0, 160) : 'Event recorded in queue'}</div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Events error:', e);
  }
}

async function loadMemories(query = '') {
  const container = document.getElementById('memory-list');
  if (!container) return;
  try {
    const url = query ? `/api/memories?q=${encodeURIComponent(query)}` : '/api/memories';
    const res = await fetch(url);
    if (!res.ok) return;
    const memories = await res.json();
    if (memories.length === 0) {
      container.innerHTML = '<div class="empty-state">No active memories found.</div>';
      return;
    }
    container.innerHTML = memories.map(m => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${m.memory_type}</span> <span class="item-tag">${m.scope}</span></span>
          <span>Confidence: ${(m.confidence * 100).toFixed(0)}%</span>
        </div>
        <div class="item-title">${escapeHtml(m.summary)}</div>
        <div class="item-content">${escapeHtml(m.content)}</div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Memories error:', e);
  }
}

async function loadTraces() {
  const container = document.getElementById('trace-list');
  if (!container) return;
  try {
    const res = await fetch('/api/traces');
    if (!res.ok) return;
    const traces = await res.json();
    if (traces.length === 0) {
      container.innerHTML = '<div class="empty-state">No repeatable success traces recorded yet.</div>';
      return;
    }
    container.innerHTML = traces.map(t => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${t.category}</span> ${t.status}</span>
          <span>Score: ${(t.reproducibility_score * 100).toFixed(0)}%</span>
        </div>
        <div class="item-title">${escapeHtml(t.title)}</div>
        <div class="item-content">
          ${t.steps.map(s => `<div><strong>${s.position}. ${escapeHtml(s.action)}</strong> (${escapeHtml(s.tool)})</div>`).join('')}
        </div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Traces error:', e);
  }
}

async function loadFailures() {
  const container = document.getElementById('failure-list');
  if (!container) return;
  try {
    const res = await fetch('/api/failures');
    if (!res.ok) return;
    const failures = await res.json();
    if (failures.length === 0) {
      container.innerHTML = '<div class="empty-state">No failure records recorded yet. All systems clean.</div>';
      return;
    }
    container.innerHTML = failures.map(f => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag" style="border-color: ${f.status === 'resolved' ? 'var(--color-green)' : 'var(--color-red)'}; color: ${f.status === 'resolved' ? 'var(--color-green)' : 'var(--color-red)'}">${f.status}</span> Tool: ${f.failed_tool || 'unknown'}</span>
          <span>${new Date(f.updated_at).toLocaleDateString()}</span>
        </div>
        <div class="item-title">${escapeHtml(f.task_title)}</div>
        <div class="item-content">
          <div style="color: var(--color-red); margin-bottom: 6px;">${escapeHtml(f.error_message)}</div>
          ${f.fix_applied ? `<div style="color: var(--color-green);">Fix: ${escapeHtml(f.fix_applied)}</div>` : ''}
        </div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Failures error:', e);
  }
}

async function loadProjects() {
  const container = document.getElementById('project-list');
  if (!container) return;
  try {
    const res = await fetch('/api/projects');
    if (!res.ok) return;
    const projects = await res.json();
    if (projects.length === 0) {
      container.innerHTML = '<div class="empty-state">No registered projects yet.</div>';
      return;
    }
    container.innerHTML = projects.map(p => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${p.slug}</span> Sync: ${p.sync_config.mode}</span>
          <span>${new Date(p.created_at).toLocaleDateString()}</span>
        </div>
        <div class="item-title">${escapeHtml(p.display_name)}</div>
        <div class="item-content">Capture enabled: ${p.capture_policy.enabled} | Raw event policy: ${p.capture_policy.raw_event_policy}</div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Projects error:', e);
  }
}

async function loadPreferences() {
  const container = document.getElementById('pref-list');
  if (!container) return;
  try {
    const res = await fetch('/api/preferences');
    if (!res.ok) return;
    const prefs = await res.json();
    if (prefs.length === 0) {
      container.innerHTML = '<div class="empty-state">No preferences configured yet.</div>';
      return;
    }
    container.innerHTML = prefs.map(p => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${p.scope}</span> ${p.locked ? '🔒 LOCKED' : 'MUTABLE'}</span>
          <span>Source: ${p.source}</span>
        </div>
        <div class="item-title">${escapeHtml(p.key)}</div>
        <div class="item-content"><code>${escapeHtml(JSON.stringify(p.value))}</code></div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Preferences error:', e);
  }
}

async function runRecallTest() {
  const queryInput = document.getElementById('test-recall-query');
  const resultArea = document.getElementById('test-recall-result');
  if (!queryInput || !resultArea) return;
  const q = queryInput.value.trim();
  if (!q) return;

  resultArea.innerText = 'Assembling active context...';
  try {
    const res = await fetch(`/api/context?q=${encodeURIComponent(q)}`);
    if (!res.ok) throw new Error('API error');
    const text = await res.text();
    resultArea.innerText = text;
  } catch (e) {
    resultArea.innerText = 'Failed to recall context: ' + e.message;
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
