const API_BASE = '';

let currentController = null;

function abortPending() {
    if (currentController) currentController.abort();
    currentController = new AbortController();
    return currentController;
}

function getHeaders() {
    var headers = {};
    var token = sessionStorage.getItem('suture-token');
    if (token) {
        headers['Authorization'] = 'Bearer ' + token;
    }
    return headers;
}

function safeId(s) {
    return s.replace(/[^a-zA-Z0-9_-]/g, '_');
}

async function fetchJSON(url, opts) {
    var signal = currentController ? currentController.signal : undefined;
    var res = await fetch(url, { ...opts, signal, headers: { ...getHeaders(), ...(opts.headers || {}) } });
    if (!res.ok) {
        var body = await res.text().catch(function () { return ''; });
        throw new Error('HTTP ' + res.status + ': ' + (body || res.statusText));
    }
    var ct = res.headers.get('content-type') || '';
    if (ct.indexOf('application/json') === -1) throw new Error('Expected JSON, got ' + ct);
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

function toast(message, type) {
    type = type || 'info';
    const container = document.getElementById('toast-container');
    const el = document.createElement('div');
    el.className = 'toast ' + type;
    el.textContent = message;
    container.appendChild(el);
    setTimeout(function () {
        el.classList.add('fade-out');
        setTimeout(function () { el.remove(); }, 300);
    }, 3500);
}

function confirmDialog(title, message) {
    return new Promise(function (resolve) {
        const overlay = document.createElement('div');
        overlay.className = 'confirm-overlay';
        overlay.innerHTML =
            '<div class="confirm-dialog">' +
            '<h3>' + escapeHtml(title) + '</h3>' +
            '<p>' + escapeHtml(message) + '</p>' +
            '<div class="confirm-actions">' +
            '<button class="btn btn-secondary confirm-cancel">Cancel</button>' +
            '<button class="btn btn-danger confirm-ok">Confirm</button>' +
            '</div></div>';
        document.body.appendChild(overlay);

        overlay.querySelector('.confirm-cancel').addEventListener('click', function () {
            overlay.remove();
            resolve(false);
        });
        overlay.querySelector('.confirm-ok').addEventListener('click', function () {
            overlay.remove();
            resolve(true);
        });
        overlay.addEventListener('click', function (e) {
            if (e.target === overlay) {
                overlay.remove();
                resolve(false);
            }
        });
        overlay.querySelector('.confirm-ok').focus();
    });
}

function switchTab(tab) {
    abortPending();
    document.querySelectorAll('.tab').forEach(function (b) {
        var isActive = b.dataset.tab === tab;
        b.classList.toggle('active', isActive);
        b.setAttribute('aria-selected', isActive ? 'true' : 'false');
    });
    document.querySelectorAll('.tab-content').forEach(function (s) {
        s.classList.toggle('active', s.id === 'tab-' + tab);
    });

    if (tab === 'dashboard') loadDashboard();
    if (tab === 'repos') loadRepos();
    if (tab === 'users') loadUsers();
    if (tab === 'mirrors') loadMirrors();
    if (tab === 'replication') loadReplication();
    if (tab === 'settings') loadSettings();
}

async function checkConnection() {
    var indicator = document.getElementById('connection-indicator');
    try {
        var data = await fetchJSON(API_BASE + '/handshake', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ client_version: 1, client_name: 'web-ui' }),
        });
        if (data.compatible) {
            indicator.className = 'indicator connected';
            indicator.title = 'Connected';
            indicator.querySelector('.indicator-text').textContent = 'Connected';
        } else {
            indicator.className = 'indicator disconnected';
            indicator.title = 'Incompatible version';
            indicator.querySelector('.indicator-text').textContent = 'Incompatible';
        }
    } catch {
        indicator.className = 'indicator disconnected';
        indicator.title = 'Disconnected';
        indicator.querySelector('.indicator-text').textContent = 'Disconnected';
    }
}

async function loadDashboard() {
    var skeleton = document.getElementById('dashboard-skeleton');
    show('dashboard-skeleton');
    try {
        var reposData = await fetchJSON(API_BASE + '/repos');
        var repoIds = reposData.repo_ids || [];

        var totalPatches = 0;
        var repoInfos = await Promise.all(
            repoIds.map(function (id) {
                return fetchJSON(API_BASE + '/repo/' + encodeURIComponent(id)).catch(function () { return null; });
            })
        );
        var repos = [];
        for (var i = 0; i < repoInfos.length; i++) {
            if (repoInfos[i] && repoInfos[i].success) {
                repos.push(repoInfos[i]);
                totalPatches += (repoInfos[i].patch_count || 0);
            }
        }

        var usersData = await fetchJSON(API_BASE + '/users');
        var userCount = (usersData.users || []).length;

        hide('dashboard-skeleton');
        document.getElementById('stat-repos').textContent = repos.length;
        document.getElementById('stat-patches').textContent = totalPatches;
        document.getElementById('stat-users').textContent = userCount;
    } catch (err) {
        hide('dashboard-skeleton');
        document.getElementById('stat-repos').textContent = '—';
        document.getElementById('stat-patches').textContent = '—';
        document.getElementById('stat-users').textContent = '—';
    }
}

async function loadRepos() {
    hide('repos-error');
    hide('repos-table-wrap');
    hide('repos-empty');
    show('repos-loading');

    try {
        var data = await fetchJSON(API_BASE + '/repos');
        var repoIds = data.repo_ids || [];

        var repoInfos = await Promise.all(
            repoIds.map(function (id) {
                return fetchJSON(API_BASE + '/repo/' + encodeURIComponent(id)).catch(function () {
                    return { repo_id: id, patch_count: 0, branches: [], success: true, error: null };
                });
            })
        );
        var repos = repoInfos.filter(function (r) { return r.success; });

        hide('repos-loading');
        document.getElementById('repo-count').textContent = repos.length + (repos.length === 1 ? ' repo' : ' repos');

        if (repos.length === 0) {
            show('repos-empty');
            return;
        }

        var tbody = document.getElementById('repos-tbody');
        tbody.innerHTML = '';

        repos.forEach(function (repo) {
            var tr = document.createElement('tr');
            tr.innerHTML =
                '<td class="mono">' + escapeHtml(repo.repo_id) + '</td>' +
                '<td>' + repo.patch_count + '</td>' +
                '<td>' + repo.branches.length + '</td>' +
                '<td><button class="expand-btn" data-repo="' + escapeHtml(repo.repo_id) + '">Details</button></td>';
            tbody.appendChild(tr);
        });

        show('repos-table-wrap');
    } catch (err) {
        hide('repos-loading');
        var banner = document.getElementById('repos-error');
        banner.textContent = 'Failed to load repositories: ' + err.message;
        show('repos-error');
    }
}

document.getElementById('repos-tbody').addEventListener('click', async function (e) {
    var btn = e.target.closest('.expand-btn');
    if (!btn) return;

    var repoId = btn.dataset.repo;
    var existing = document.getElementById('detail-' + safeId(repoId));

    if (existing) {
        existing.remove();
        btn.textContent = 'Details';
        return;
    }

    try {
        var info = await fetchJSON(API_BASE + '/repo/' + encodeURIComponent(repoId));
        var branches = (info.branches || [])
            .map(function (b) {
                var prot = b.protected ? 'protected' : 'unprotected';
                var label = b.protected ? '🔒' : '🔓';
                return '<span class="branch-tag">' + escapeHtml(b.name) +
                    '<span class="protection-badge ' + prot + '" data-repo="' + escapeHtml(repoId) +
                    '" data-branch="' + escapeHtml(b.name) + '" title="Toggle protection">' +
                    label + '</span></span>';
            })
            .join('');

        var patchesHtml = '<span style="color:var(--text-secondary)">none</span>';
        try {
            var patchesData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/patches?limit=10');
            var patches = patchesData.patches || [];
            if (patches.length > 0) {
                patchesHtml = '<ul class="patches-list">' +
                    patches.map(function (p) {
                        return '<li><span class="patch-id">' + escapeHtml(p.patch_id || p.id || '') + '</span>' +
                            (p.description ? ' — ' + escapeHtml(p.description) : '') + '</li>';
                    }).join('') +
                    '</ul>';
            }
        } catch { /* patches endpoint may not exist yet */ }

        var detailsTr = document.createElement('tr');
        detailsTr.id = 'detail-' + safeId(repoId);
        detailsTr.className = 'repo-details';
        detailsTr.innerHTML =
            '<td colspan="4">' +
            '<div class="detail-label">Branches <span style="font-weight:400;text-transform:none;letter-spacing:0">(click lock to toggle protection)</span></div>' +
            '<div class="detail-value">' + (branches || '<span style="color:var(--text-secondary)">none</span>') + '</div>' +
            '<div class="detail-label">Recent Patches</div>' +
            '<div class="detail-value">' + patchesHtml + '</div>' +
            '<div class="detail-label">Patch Count</div>' +
            '<div class="detail-value">' + info.patch_count + '</div>' +
            '</td>';

        var row = btn.closest('tr');
        row.after(detailsTr);
        btn.textContent = 'Hide';
    } catch (err) {
        console.error('Failed to load repo details:', err);
    }
});

document.addEventListener('click', async function (e) {
    var badge = e.target.closest('.protection-badge');
    if (!badge) return;

    var repoId = badge.dataset.repo;
    var branchName = badge.dataset.branch;
    var isProtected = badge.classList.contains('protected');
    var action = isProtected ? 'unprotect' : 'protect';

    try {
        await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/branches/' +
            encodeURIComponent(branchName) + '/' + action, { method: 'POST' });
        toast('Branch "' + branchName + '" ' + (isProtected ? 'unprotected' : 'protected'), 'success');
        badge.classList.toggle('protected', !isProtected);
        badge.classList.toggle('unprotected', isProtected);
        badge.textContent = isProtected ? '🔓' : '🔒';
    } catch (err) {
        toast('Failed to toggle protection: ' + err.message, 'error');
    }
});

document.getElementById('btn-create-repo').addEventListener('click', function () {
    togglePanel('create-repo-form');
});

document.getElementById('cancel-create-repo').addEventListener('click', function () {
    hide('create-repo-form');
});

document.getElementById('form-create-repo').addEventListener('submit', async function (e) {
    e.preventDefault();
    var name = document.getElementById('new-repo-name').value.trim();
    if (!name) return;

    try {
        await fetchJSON(API_BASE + '/push', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ repo_id: name, patches: [] }),
        });
        toast('Repository "' + name + '" created', 'success');
        document.getElementById('form-create-repo').reset();
        hide('create-repo-form');
        loadRepos();
    } catch (err) {
        toast('Failed to create repository: ' + err.message, 'error');
    }
});

async function loadUsers() {
    hide('users-error');
    hide('users-table-wrap');
    hide('users-empty');
    show('users-loading');

    try {
        var data = await fetchJSON(API_BASE + '/users');
        var users = data.users || [];

        hide('users-loading');
        document.getElementById('user-count').textContent = users.length + (users.length === 1 ? ' user' : ' users');

        if (users.length === 0) {
            show('users-empty');
            return;
        }

        var tbody = document.getElementById('users-tbody');
        tbody.innerHTML = '';

        users.forEach(function (user) {
            var tr = document.createElement('tr');
            var role = (user.role || 'user').toLowerCase();
            tr.innerHTML =
                '<td class="mono">' + escapeHtml(user.username) + '</td>' +
                '<td>' + escapeHtml(user.display_name) + '</td>' +
                '<td><select class="role-select" data-user="' + escapeHtml(user.username) + '">' +
                '<option value="admin"' + (role === 'admin' ? ' selected' : '') + '>admin</option>' +
                '<option value="user"' + (role === 'user' ? ' selected' : '') + '>user</option>' +
                '<option value="readonly"' + (role === 'readonly' ? ' selected' : '') + '>readonly</option>' +
                '</select></td>' +
                '<td>' + formatTimestamp(user.created_at) + '</td>' +
                '<td class="actions-cell">' +
                '<button class="btn btn-danger btn-sm user-delete" data-user="' + escapeHtml(user.username) + '">Delete</button>' +
                '</td>';
            tbody.appendChild(tr);
        });

        show('users-table-wrap');
    } catch (err) {
        hide('users-loading');
        var banner = document.getElementById('users-error');
        banner.textContent = 'Failed to load users: ' + err.message;
        show('users-error');
    }
}

document.getElementById('btn-toggle-create-user').addEventListener('click', function () {
    togglePanel('create-user-form');
});

document.getElementById('cancel-create-user').addEventListener('click', function () {
    hide('create-user-form');
});

document.getElementById('form-create-user').addEventListener('submit', async function (e) {
    e.preventDefault();
    var username = document.getElementById('new-user-username').value.trim();
    var displayName = document.getElementById('new-user-display').value.trim();
    var password = document.getElementById('new-user-password').value;
    if (!username || !password) return;

    try {
        await fetchJSON(API_BASE + '/auth/register', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                username: username,
                display_name: displayName || username,
                password: password,
            }),
        });
        toast('User "' + username + '" created', 'success');
        document.getElementById('form-create-user').reset();
        hide('create-user-form');
        loadUsers();
    } catch (err) {
        toast('Failed to create user: ' + err.message, 'error');
    }
});

document.getElementById('users-tbody').addEventListener('change', async function (e) {
    var sel = e.target.closest('.role-select');
    if (!sel) return;

    var username = sel.dataset.user;
    var newRole = sel.value;

    try {
        await fetchJSON(API_BASE + '/users/' + encodeURIComponent(username) + '/role', {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ role: newRole }),
        });
        toast('Role for "' + username + '" updated to ' + newRole, 'success');
    } catch (err) {
        toast('Failed to update role: ' + err.message, 'error');
        loadUsers();
    }
});

document.getElementById('users-tbody').addEventListener('click', async function (e) {
    var btn = e.target.closest('.user-delete');
    if (!btn) return;

    var username = btn.dataset.user;
    var ok = await confirmDialog('Delete User', 'Are you sure you want to delete user "' + username + '"? This cannot be undone.');
    if (!ok) return;

    try {
        await fetchJSON(API_BASE + '/users/' + encodeURIComponent(username), {
            method: 'DELETE',
        });
        toast('User "' + username + '" deleted', 'success');
        loadUsers();
    } catch (err) {
        toast('Failed to delete user: ' + err.message, 'error');
    }
});

async function loadMirrors() {
    hide('mirrors-error');
    hide('mirrors-table-wrap');
    hide('mirrors-empty');
    show('mirrors-loading');

    try {
        var data = await fetchJSON(API_BASE + '/mirror/status');
        var mirrors = data.mirrors || [];

        hide('mirrors-loading');
        document.getElementById('mirror-count').textContent = mirrors.length + (mirrors.length === 1 ? ' mirror' : ' mirrors');

        if (mirrors.length === 0) {
            show('mirrors-empty');
            return;
        }

        var tbody = document.getElementById('mirrors-tbody');
        tbody.innerHTML = '';

        mirrors.forEach(function (m) {
            var tr = document.createElement('tr');
            tr.innerHTML =
                '<td class="mono">' + escapeHtml(m.local_repo || m.repo_id || '') + '</td>' +
                '<td class="mono">' + escapeHtml(m.remote_url || m.url || '') + '</td>' +
                '<td>' + formatTimestamp(m.last_sync) + '</td>' +
                '<td class="actions-cell">' +
                '<button class="btn btn-primary btn-sm mirror-sync" data-repo="' + escapeHtml(m.local_repo || m.repo_id || '') +
                '" data-url="' + escapeHtml(m.remote_url || m.url || '') + '">Sync</button>' +
                '</td>';
            tbody.appendChild(tr);
        });

        show('mirrors-table-wrap');
    } catch (err) {
        hide('mirrors-loading');
        var banner = document.getElementById('mirrors-error');
        banner.textContent = 'Failed to load mirrors: ' + err.message;
        show('mirrors-error');
    }
}

document.getElementById('btn-toggle-add-mirror').addEventListener('click', function () {
    togglePanel('add-mirror-form');
});

document.getElementById('cancel-add-mirror').addEventListener('click', function () {
    hide('add-mirror-form');
});

document.getElementById('form-add-mirror').addEventListener('submit', async function (e) {
    e.preventDefault();
    var remoteUrl = document.getElementById('mirror-remote-url').value.trim();
    var localRepo = document.getElementById('mirror-local-repo').value.trim();
    if (!remoteUrl || !localRepo) return;

    try {
        await fetchJSON(API_BASE + '/mirror/setup', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ remote_url: remoteUrl, local_repo: localRepo }),
        });
        toast('Mirror added for "' + localRepo + '"', 'success');
        document.getElementById('form-add-mirror').reset();
        hide('add-mirror-form');
        loadMirrors();
    } catch (err) {
        toast('Failed to add mirror: ' + err.message, 'error');
    }
});

document.getElementById('mirrors-tbody').addEventListener('click', async function (e) {
    var btn = e.target.closest('.mirror-sync');
    if (!btn) return;

    var repo = btn.dataset.repo;
    var url = btn.dataset.url;
    btn.disabled = true;
    btn.textContent = 'Syncing...';

    try {
        await fetchJSON(API_BASE + '/mirror/sync', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ local_repo: repo, remote_url: url }),
        });
        toast('Sync completed for "' + repo + '"', 'success');
        loadMirrors();
    } catch (err) {
        toast('Sync failed: ' + err.message, 'error');
        btn.disabled = false;
        btn.textContent = 'Sync';
    }
});

async function loadReplication() {
    hide('replication-error');
    hide('replication-info-wrap');
    hide('replication-empty');
    show('replication-loading');

    try {
        var results = await Promise.all([
            fetchJSON(API_BASE + '/replication/status'),
            fetchJSON(API_BASE + '/replication/peers'),
        ]);
        var statusData = results[0];
        var peersData = results[1];

        hide('replication-loading');

        var status = statusData.status || {};
        var peers = peersData.peers || [];
        var currentSeq = status.current_seq || 0;
        var peerCount = status.peer_count || peers.length;

        document.getElementById('replication-role-badge').textContent = 'seq ' + currentSeq;

        if (peers.length === 0 && currentSeq === 0) {
            show('replication-empty');
            return;
        }

        document.getElementById('replication-current-seq').textContent = currentSeq;
        document.getElementById('replication-peer-count').textContent = peerCount;

        var tbody = document.getElementById('replication-tbody');
        tbody.innerHTML = '';

        if (peers.length === 0) {
            var tr = document.createElement('tr');
            tr.innerHTML = '<td colspan="5" style="text-align:center;color:var(--text-secondary)">No peers configured</td>';
            tbody.appendChild(tr);
        } else {
            peers.forEach(function (peer) {
                var tr = document.createElement('tr');
                var peerStatus = (peer.status || 'active').toLowerCase();
                tr.innerHTML =
                    '<td class="mono">' + escapeHtml(peer.peer_url) + '</td>' +
                    '<td><span class="role-badge ' + escapeHtml(peer.role) + '">' + escapeHtml(peer.role) + '</span></td>' +
                    '<td><span class="status-badge ' + peerStatus + '">' + escapeHtml(peerStatus) + '</span></td>' +
                    '<td class="mono">' + peer.last_sync_seq + '</td>' +
                    '<td class="actions-cell">' +
                    '<button class="btn btn-danger btn-sm peer-remove" data-url="' + escapeHtml(peer.peer_url) + '">Remove</button>' +
                    '</td>';
                tbody.appendChild(tr);
            });
        }

        show('replication-info-wrap');
    } catch (err) {
        hide('replication-loading');
        var banner = document.getElementById('replication-error');
        banner.textContent = 'Failed to load replication status: ' + err.message;
        show('replication-error');
    }
}

document.getElementById('btn-toggle-add-peer').addEventListener('click', function () {
    togglePanel('add-peer-form');
});

document.getElementById('cancel-add-peer').addEventListener('click', function () {
    hide('add-peer-form');
});

document.getElementById('form-add-peer').addEventListener('submit', async function (e) {
    e.preventDefault();
    var url = document.getElementById('peer-url').value.trim();
    var role = document.getElementById('peer-role').value;
    if (!url) return;

    try {
        await fetchJSON(API_BASE + '/replication/peers', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ peer_url: url, role: role }),
        });
        toast('Peer "' + url + '" added', 'success');
        document.getElementById('form-add-peer').reset();
        hide('add-peer-form');
        loadReplication();
    } catch (err) {
        toast('Failed to add peer: ' + err.message, 'error');
    }
});

document.getElementById('replication-tbody').addEventListener('click', async function (e) {
    var btn = e.target.closest('.peer-remove');
    if (!btn) return;

    var url = btn.dataset.url;
    var ok = await confirmDialog('Remove Peer', 'Are you sure you want to remove peer "' + url + '"?');
    if (!ok) return;

    try {
        await fetchJSON(API_BASE + '/replication/peers/' + encodeURIComponent(url), {
            method: 'DELETE',
        });
        toast('Peer removed', 'success');
        loadReplication();
    } catch (err) {
        toast('Failed to remove peer: ' + err.message, 'error');
    }
});

async function loadSettings() {
    hide('settings-content');
    show('settings-loading');

    try {
        var handshake = await fetchJSON(API_BASE + '/handshake', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ client_version: 1, client_name: 'web-ui' }),
        });

        var replData = null;
        try {
            replData = await fetchJSON(API_BASE + '/replication/status');
        } catch { /* standalone */ }

        hide('settings-loading');
        document.getElementById('setting-version').textContent = handshake.server_version || handshake.version || 'unknown';
        document.getElementById('setting-replication-role').textContent = (replData && replData.status && replData.status.role) || 'standalone';
        document.getElementById('setting-auth-mode').textContent = handshake.auth_mode || handshake.auth_required !== undefined
            ? (handshake.auth_required ? 'enabled' : 'disabled')
            : 'unknown';
        show('settings-content');
    } catch (err) {
        hide('settings-loading');
        document.getElementById('setting-version').textContent = 'error';
        document.getElementById('setting-replication-role').textContent = '—';
        document.getElementById('setting-auth-mode').textContent = '—';
        show('settings-content');
    }
}

function togglePanel(id) {
    var el = document.getElementById(id);
    if (el.classList.contains('hidden')) {
        show(id);
    } else {
        hide(id);
    }
}

document.querySelectorAll('.tab').forEach(function (btn) {
    btn.addEventListener('click', function () { switchTab(btn.dataset.tab); });
});

checkConnection();
setInterval(checkConnection, 30000);
loadDashboard();

var tokenInput = document.getElementById('token-input');
var tokenBtn = document.getElementById('token-btn');
if (tokenInput && tokenBtn) {
    var savedToken = sessionStorage.getItem('suture-token');
    if (savedToken) tokenInput.value = savedToken;
    tokenBtn.addEventListener('click', function () {
        var val = tokenInput.value.trim();
        if (val) {
            sessionStorage.setItem('suture-token', val);
        } else {
            sessionStorage.removeItem('suture-token');
        }
    });
}
