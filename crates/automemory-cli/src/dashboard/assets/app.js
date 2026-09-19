// AutoMemory Pro Dashboard 2.0 JavaScript

let currentTab = 'overview';
let sseSource = null;
let graphNodes = [];
let graphEdges = [];
let selectedNode = null;
let canvasAnimId = null;

document.addEventListener('DOMContentLoaded', () => {
  setupTabs();
  loadStats();
  loadRecentEvents();
  loadMemories();
  loadTraces();
  loadFailures();
  loadProposals();
  loadProjects();
  loadPreferences();
  initSSE();
  initGraphCanvas();

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

      if (currentTab === 'graph') {
        reloadGraph();
      } else if (currentTab === 'proposals') {
        loadProposals();
      }
    });
  });
}

function initSSE() {
  try {
    sseSource = new EventSource('/api/stream');
    sseSource.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
        if (payload.type === 'pulse') {
          document.getElementById('stat-active-memories').innerText = payload.active_memories || 0;
          document.getElementById('stat-proposals').innerText = payload.pending_proposals || 0;
          if (currentTab === 'overview' && payload.recent_events) {
            renderEvents(payload.recent_events);
          }
        }
      } catch (err) {
        console.error('SSE parse error:', err);
      }
    };
    sseSource.onerror = () => {
      document.getElementById('sse-status').innerText = 'Reconnecting Stream...';
    };
    sseSource.onopen = () => {
      document.getElementById('sse-status').innerText = 'Live Stream Active';
    };
  } catch (e) {
    console.error('SSE initialization error:', e);
  }
}

async function loadStats() {
  try {
    const res = await fetch('/api/stats');
    if (!res.ok) return;
    const data = await res.json();
    document.getElementById('stat-active-memories').innerText = data.active_memories || 0;
    document.getElementById('stat-traces').innerText = data.active_traces || 0;
    document.getElementById('stat-failures').innerText = data.unresolved_failures || 0;
    document.getElementById('stat-proposals').innerText = data.total_projects || 0;
  } catch (e) {
    console.error('Stats error:', e);
  }
}

async function loadRecentEvents() {
  try {
    const res = await fetch('/api/events');
    if (!res.ok) return;
    const events = await res.json();
    renderEvents(events);
  } catch (e) {
    console.error('Events error:', e);
  }
}

function renderEvents(events) {
  const container = document.getElementById('event-list');
  if (!container) return;
  if (!events || events.length === 0) {
    container.innerHTML = '<div class="empty-state">No events captured yet. Run an agent turn to see live hooks!</div>';
    return;
  }
  container.innerHTML = events.map(e => `
    <div class="item-card">
      <div class="item-meta">
        <span><span class="item-tag">${e.agent.adapter_id}</span> ${e.event_type}</span>
        <span>${new Date(e.timestamp).toLocaleTimeString()}</span>
      </div>
      <div class="item-title">${e.tool ? e.tool.name : (e.task_id || e.session_id)}</div>
      <div class="item-content">${e.tool && e.tool.input ? JSON.stringify(e.tool.input).slice(0, 160) : 'Event stored in durable ledger'}</div>
    </div>
  `).join('');
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
      container.innerHTML = '<div class="empty-state">No failure records. All task runs clean!</div>';
      return;
    }
    container.innerHTML = failures.map(f => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag" style="border-color: var(--color-red); color: var(--color-red);">${f.status}</span> ${f.failed_tool || 'Task'}</span>
          <span>${new Date(f.created_at).toLocaleDateString()}</span>
        </div>
        <div class="item-title">${escapeHtml(f.task_title)}</div>
        <div class="item-content">
          <div style="color: var(--color-red); margin-bottom: 4px;"><strong>Error:</strong> ${escapeHtml(f.error_message)}</div>
          ${f.fix_applied ? `<div style="color: var(--color-green);"><strong>Fix:</strong> ${escapeHtml(f.fix_applied)}</div>` : ''}
        </div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Failures error:', e);
  }
}

async function loadProposals() {
  const container = document.getElementById('proposal-list');
  if (!container) return;
  try {
    const res = await fetch('/api/proposals');
    if (!res.ok) return;
    const proposals = await res.json();
    if (proposals.length === 0) {
      container.innerHTML = '<div class="empty-state">No pending review proposals. Vault is consolidated!</div>';
      return;
    }
    container.innerHTML = proposals.map(p => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag" style="border-color: var(--color-gold); color: var(--color-gold);">${p.proposal_type}</span> ${p.status}</span>
          <span>${new Date(p.created_at).toLocaleDateString()}</span>
        </div>
        <div class="item-title">${escapeHtml(p.description)}</div>
        <div class="item-content" style="margin-bottom: 12px;"><strong>Suggested Action:</strong> ${escapeHtml(p.suggested_action)}</div>
        <div style="display: flex; gap: 8px;">
          <button class="btn-approve" onclick="decideProposal('${p.id}', true)">Approve</button>
          <button class="btn-reject" onclick="decideProposal('${p.id}', false)">Reject</button>
        </div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Proposals error:', e);
  }
}

async function decideProposal(id, approved) {
  try {
    const res = await fetch('/api/proposals/decide', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, approved }),
    });
    if (res.ok) {
      loadProposals();
      loadStats();
    }
  } catch (e) {
    console.error('Decide error:', e);
  }
}

async function loadProjects() {
  const container = document.getElementById('project-list');
  if (!container) return;
  try {
    const res = await fetch('/api/projects');
    if (!res.ok) return;
    const projects = await res.json();
    const docsRes = await fetch('/api/docs');
    const docs = docsRes.ok ? await docsRes.json() : [];

    if (projects.length === 0) {
      container.innerHTML = '<div class="empty-state">No registered projects yet. Run "automemory init" to register!</div>';
      return;
    }
    container.innerHTML = projects.map(p => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${p.slug}</span> Sync: ${p.sync_config.mode}</span>
          <span>Created: ${new Date(p.created_at).toLocaleDateString()}</span>
        </div>
        <div class="item-title">${escapeHtml(p.display_name)}</div>
        <div class="item-content">
          <div><strong>Capture Policy:</strong> ${p.capture_policy.auto_capture ? 'Automatic' : 'Manual'}</div>
          <div><strong>Tracked Docs:</strong> ${docs.filter(d => d.project_id === p.id).map(d => d.relative_path).join(', ') || 'None'}</div>
        </div>
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
      container.innerHTML = '<div class="empty-state">No preferences configured.</div>';
      return;
    }
    container.innerHTML = prefs.map(p => `
      <div class="item-card">
        <div class="item-meta">
          <span><span class="item-tag">${p.scope}</span> ${p.locked ? '🔒 Locked' : 'Unlocked'}</span>
          <span>Source: ${p.source}</span>
        </div>
        <div class="item-title">${escapeHtml(p.key)}</div>
        <div class="item-content">${escapeHtml(typeof p.value === 'object' ? JSON.stringify(p.value) : String(p.value))}</div>
      </div>
    `).join('');
  } catch (e) {
    console.error('Preferences error:', e);
  }
}

async function runRecallTest() {
  const query = document.getElementById('test-recall-query').value.trim();
  const output = document.getElementById('test-recall-result');
  output.innerText = 'Assembling bounded context bundle via RRF Hybrid Retrieval...';

  try {
    const url = query ? `/api/context?q=${encodeURIComponent(query)}` : '/api/context';
    const res = await fetch(url);
    if (!res.ok) {
      output.innerText = 'Failed to assemble context.';
      return;
    }
    const text = await res.text();
    output.innerText = text;
  } catch (e) {
    output.innerText = 'Error calling context assembler: ' + e;
  }
}

// ==========================================
// Knowledge Graph Interactive Visualizer
// ==========================================
function initGraphCanvas() {
  const canvas = document.getElementById('graph-canvas');
  if (!canvas) return;

  canvas.addEventListener('click', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    for (let node of graphNodes) {
      const dx = node.x - x;
      const dy = node.y - y;
      if (Math.sqrt(dx * dx + dy * dy) < node.radius + 4) {
        selectedNode = node;
        updateGraphOverlay(node);
        break;
      }
    }
  });
}

async function reloadGraph() {
  try {
    const res = await fetch('/api/graph');
    if (!res.ok) return;
    const data = await res.json();
    buildGraphSimulation(data.entities || [], data.edges || []);
  } catch (e) {
    console.error('Graph reload error:', e);
  }
}

function buildGraphSimulation(entities, edges) {
  const canvas = document.getElementById('graph-canvas');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const width = canvas.width;
  const height = canvas.height;

  // Initialize node coordinates
  graphNodes = entities.map((ent, idx) => {
    const angle = (idx / Math.max(entities.len, 1)) * 2 * Math.PI;
    const r = 120 + Math.random() * 100;
    return {
      id: ent.id,
      name: ent.name,
      type: ent.entity_type,
      path: ent.path,
      x: width / 2 + Math.cos(angle) * r,
      y: height / 2 + Math.sin(angle) * r,
      vx: 0,
      vy: 0,
      radius: ent.entity_type === 'file' ? 12 : (ent.entity_type === 'doc' ? 10 : 8),
      color: ent.entity_type === 'file' ? '#ffc000' : (ent.entity_type === 'function' ? '#29abe2' : '#2fb344'),
    };
  });

  graphEdges = edges;

  if (canvasAnimId) {
    cancelAnimationFrame(canvasAnimId);
  }

  function tick() {
    // Simple force layout
    for (let i = 0; i < graphNodes.length; i++) {
      for (let j = i + 1; j < graphNodes.length; j++) {
        const n1 = graphNodes[i];
        const n2 = graphNodes[j];
        const dx = n2.x - n1.x;
        const dy = n2.y - n1.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        if (dist < 80) {
          const force = (80 - dist) / 80 * 0.5;
          n1.vx -= (dx / dist) * force;
          n1.vy -= (dy / dist) * force;
          n2.vx += (dx / dist) * force;
          n2.vy += (dy / dist) * force;
        }
      }
    }

    // Center gravity
    for (let n of graphNodes) {
      n.vx += (width / 2 - n.x) * 0.005;
      n.vy += (height / 2 - n.y) * 0.005;
      n.x += n.vx;
      n.y += n.vy;
      n.vx *= 0.85;
      n.vy *= 0.85;
    }

    // Render frame
    ctx.clearRect(0, 0, width, height);

    // Draw edges
    ctx.strokeStyle = '#262626';
    ctx.lineWidth = 1;
    for (let edge of graphEdges) {
      const src = graphNodes.find(n => n.id === edge.source_id);
      const tgt = graphNodes.find(n => n.id === edge.target_id);
      if (src && tgt) {
        ctx.beginPath();
        ctx.moveTo(src.x, src.y);
        ctx.lineTo(tgt.x, tgt.y);
        ctx.stroke();
      }
    }

    // Draw nodes
    for (let n of graphNodes) {
      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = n.color;
      ctx.fill();

      // Outline selected node
      if (selectedNode && selectedNode.id === n.id) {
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      // Draw label
      ctx.fillStyle = '#f5f5f5';
      ctx.font = '10px monospace';
      ctx.fillText(n.name.slice(0, 16), n.x + n.radius + 3, n.y + 3);
    }

    canvasAnimId = requestAnimationFrame(tick);
  }

  tick();
}

function updateGraphOverlay(node) {
  document.getElementById('graph-selected-title').innerText = `${node.type.toUpperCase()}: ${node.name}`;
  document.getElementById('graph-selected-desc').innerText = `Path: ${node.path}\nNode ID: ${node.id.slice(0, 8)}...`;
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
