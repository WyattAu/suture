const API_BASE = '';

async function fetchJSON(url, opts) {
    const res = await fetch(url, opts);
    if (!res.ok) {
        const body = await res.text().catch(() => '');
        throw new Error(`HTTP ${res.status}: ${res.statusText}${body ? ' — ' + body : ''}`);
    }
    return res.json();
}

function escapeHtml(str) {
    if (!str) return '';
    const el = document.createElement('span');
    el.textContent = str;
    return el.innerHTML;
}

function formatTimestamp(ts) {
    if (!ts) return '—';
    const d = new Date(Number(ts) * 1000);
    if (isNaN(d.getTime())) return '—';
    return d.toLocaleString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

function show(id) {
    const el = document.getElementById(id);
    if (el) el.classList.remove('hidden');
}

function hide(id) {
    const el = document.getElementById(id);
    if (el) el.classList.add('hidden');
}

function switchTab(tab) {
    document.querySelectorAll('.tab').forEach(b => b.classList.toggle('active', b.dataset.tab === tab));
    document.querySelectorAll('.tab-content').forEach(s => s.classList.toggle('active', s.id === 'tab-' + tab));

    if (tab === 'repos') loadRepos();
    if (tab === 'users') loadUsers();
    if (tab === 'replication') loadReplication();
}

async function checkConnection() {
    const indicator = document.getElementById('connection-indicator');
    try {
        await fetchJSON(`${API_BASE}/handshake`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ client_version: 1, client_name: 'web-ui' }),
        });
        indicator.className = 'indicator connected';
        indicator.title = 'Connected';
        indicator.querySelector('.indicator-text').textContent = 'Connected';
    } catch {
        indicator.className = 'indicator disconnected';
        indicator.title = 'Disconnected';
        indicator.querySelector('.indicator-text').textContent = 'Disconnected';
    }
}

async function loadRepos() {
    hide('repos-error');
    hide('repos-table-wrap');
    hide('repos-empty');
    show('repos-loading');

    try {
        const data = await fetchJSON(`${API_BASE}/repos`);
        const repoIds = data.repo_ids || [];

        const repos = [];
        for (const id of repoIds) {
            try {
                const info = await fetchJSON(`${API_BASE}/repo/${encodeURIComponent(id)}`);
                if (info.success) repos.push(info);
            } catch {
                repos.push({ repo_id: id, patch_count: 0, branches: [], success: true, error: null });
            }
        }

        hide('repos-loading');
        document.getElementById('repo-count').textContent = repos.length + (repos.length === 1 ? ' repo' : ' repos');

        if (repos.length === 0) {
            show('repos-empty');
            return;
        }

        const tbody = document.getElementById('repos-tbody');
        tbody.innerHTML = '';

        repos.forEach(repo => {
            const tr = document.createElement('tr');
            tr.innerHTML = `
                <td class="mono">${escapeHtml(repo.repo_id)}</td>
                <td>${repo.patch_count}</td>
                <td>${repo.branches.length}</td>
                <td><button class="expand-btn" data-repo="${escapeHtml(repo.repo_id)}">Details</button></td>
            `;
            tbody.appendChild(tr);
        });

        show('repos-table-wrap');
    } catch (err) {
        hide('repos-loading');
        const banner = document.getElementById('repos-error');
        banner.textContent = 'Failed to load repositories: ' + err.message;
        show('repos-error');
    }
}

document.getElementById('repos-tbody').addEventListener('click', async (e) => {
    const btn = e.target.closest('.expand-btn');
    if (!btn) return;

    const repoId = btn.dataset.repo;
    const existing = document.getElementById('detail-' + CSS.escape(repoId));

    if (existing) {
        existing.remove();
        btn.textContent = 'Details';
        return;
    }

    try {
        const info = await fetchJSON(`${API_BASE}/repo/${encodeURIComponent(repoId)}`);
        const branches = (info.branches || [])
            .map(b => `<span class="branch-tag">${escapeHtml(b.name)}</span>`)
            .join('');

        const detailsTr = document.createElement('tr');
        detailsTr.id = 'detail-' + CSS.escape(repoId);
        detailsTr.className = 'repo-details';
        detailsTr.innerHTML = `
            <td colspan="4">
                <div class="detail-label">Branches</div>
                <div class="detail-value">${branches || '<span style="color:var(--text-secondary)">none</span>'}</div>
                <div class="detail-label">Patch Count</div>
                <div class="detail-value">${info.patch_count}</div>
            </td>
        `;

        const row = btn.closest('tr');
        row.after(detailsTr);
        btn.textContent = 'Hide';
    } catch (err) {
        console.error('Failed to load repo details:', err);
    }
});

async function loadUsers() {
    hide('users-error');
    hide('users-table-wrap');
    hide('users-empty');
    show('users-loading');

    try {
        const data = await fetchJSON(`${API_BASE}/users`);
        const users = data.users || [];

        hide('users-loading');
        document.getElementById('user-count').textContent = users.length + (users.length === 1 ? ' user' : ' users');

        if (users.length === 0) {
            show('users-empty');
            return;
        }

        const tbody = document.getElementById('users-tbody');
        tbody.innerHTML = '';

        users.forEach(user => {
            const tr = document.createElement('tr');
            const role = (user.role || 'user').toLowerCase();
            tr.innerHTML = `
                <td class="mono">${escapeHtml(user.username)}</td>
                <td>${escapeHtml(user.display_name)}</td>
                <td><span class="role-badge ${role}">${escapeHtml(role)}</span></td>
                <td>${formatTimestamp(user.created_at)}</td>
            `;
            tbody.appendChild(tr);
        });

        show('users-table-wrap');
    } catch (err) {
        hide('users-loading');
        const banner = document.getElementById('users-error');
        banner.textContent = 'Failed to load users: ' + err.message;
        show('users-error');
    }
}

async function loadReplication() {
    hide('replication-error');
    hide('replication-table-wrap');
    hide('replication-empty');
    show('replication-loading');

    try {
        const data = await fetchJSON(`${API_BASE}/mirror/status`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({}),
        });
        const mirrors = data.mirrors || [];

        hide('replication-loading');

        if (mirrors.length === 0) {
            show('replication-empty');
            return;
        }

        const tbody = document.getElementById('replication-tbody');
        tbody.innerHTML = '';

        mirrors.forEach(mirror => {
            const tr = document.createElement('tr');
            const status = (mirror.status || 'idle').toLowerCase();
            tr.innerHTML = `
                <td class="mono">${escapeHtml(mirror.repo_name)}</td>
                <td class="mono">${escapeHtml(mirror.upstream_url)}</td>
                <td><span class="status-badge ${status}">${escapeHtml(status)}</span></td>
                <td>${mirror.last_sync ? formatTimestamp(mirror.last_sync) : 'Never'}</td>
            `;
            tbody.appendChild(tr);
        });

        show('replication-table-wrap');
    } catch (err) {
        hide('replication-loading');
        const banner = document.getElementById('replication-error');
        banner.textContent = 'Failed to load replication status: ' + err.message;
        show('replication-error');
    }
}

document.querySelectorAll('.tab').forEach(btn => {
    btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});

checkConnection();
loadRepos();
