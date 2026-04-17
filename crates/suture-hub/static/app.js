var API_BASE = '';
var currentController = null;

function abortPending() {
    if (currentController) currentController.abort();
    currentController = new AbortController();
    return currentController;
}

function getHeaders() {
    var headers = {};
    var token = sessionStorage.getItem('suture-token');
    if (token) headers['Authorization'] = 'Bearer ' + token;
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
    var el = document.createElement('span');
    el.textContent = str;
    return el.innerHTML;
}

function formatTimestamp(ts) {
    if (!ts) return '\u2014';
    var d = new Date(Number(ts) * 1000);
    if (isNaN(d.getTime())) return '\u2014';
    return d.toLocaleString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

function show(id) {
    var el = document.getElementById(id);
    if (el) el.classList.remove('hidden');
}

function hide(id) {
    var el = document.getElementById(id);
    if (el) el.classList.add('hidden');
}

function toast(message, type) {
    type = type || 'info';
    var container = document.getElementById('toast-container');
    var el = document.createElement('div');
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
        var overlay = document.createElement('div');
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

function togglePanel(id) {
    var el = document.getElementById(id);
    if (!el) return;
    if (el.classList.contains('hidden')) show(id);
    else hide(id);
}

function isBinary(str) {
    for (var i = 0; i < Math.min(str.length, 8000); i++) {
        if (str.charCodeAt(i) === 0) return true;
    }
    return false;
}

function formatBytes(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function parseRoute() {
    var hash = (window.location.hash || '').slice(1) || '/';
    var m, params;

    if (hash === '/' || hash === '/dashboard') return { view: 'dashboard' };
    if (hash === '/repos') return { view: 'repos' };
    if (hash === '/users') return { view: 'users' };
    if (hash === '/mirrors') return { view: 'mirrors' };
    if (hash === '/replication') return { view: 'replication' };
    if (hash === '/settings') return { view: 'settings' };
    if (hash === '/login') return { view: 'login' };

    m = hash.match(/^\/repo\/([^/]+)$/);
    if (m) return { view: 'repo-detail', repoId: decodeURIComponent(m[1]) };

    m = hash.match(/^\/repo\/([^/]+)\/tree\/([^?]+)(?:\?(.*))?$/);
    if (m) {
        params = {};
        (m[3] || '').split('&').forEach(function (p) {
            var kv = p.split('=');
            if (kv[0] === 'path') params.path = decodeURIComponent(kv[1] || '');
        });
        return { view: 'file-tree', repoId: decodeURIComponent(m[1]), branch: decodeURIComponent(m[2]), path: params.path || '' };
    }

    m = hash.match(/^\/repo\/([^/]+)\/blob\/([^/]+)$/);
    if (m) return { view: 'blob', repoId: decodeURIComponent(m[1]), contentHash: decodeURIComponent(m[2]) };

    m = hash.match(/^\/repo\/([^/]+)\/patches(?:\?(.*))?$/);
    if (m) {
        params = {};
        (m[2] || '').split('&').forEach(function (p) {
            var kv = p.split('=');
            if (kv[0] === 'offset') params.offset = parseInt(kv[1]) || 0;
        });
        return { view: 'patches', repoId: decodeURIComponent(m[1]), offset: params.offset || 0 };
    }

    m = hash.match(/^\/search\?(.*)$/);
    if (m) {
        var q = '';
        m[1].split('&').forEach(function (p) {
            var kv = p.split('=');
            if (kv[0] === 'q') q = decodeURIComponent(kv[1] || '');
        });
        return { view: 'search', query: q };
    }

    return { view: 'dashboard' };
}

function getActiveTab(route) {
    var v = route.view;
    if (v === 'dashboard') return 'dashboard';
    if (v === 'repos' || v === 'repo-detail' || v === 'file-tree' || v === 'blob' || v === 'patches') return 'repos';
    if (v === 'users') return 'users';
    if (v === 'mirrors') return 'mirrors';
    if (v === 'replication') return 'replication';
    if (v === 'settings') return 'settings';
    return '';
}

function updateTabs(route) {
    var active = getActiveTab(route);
    document.querySelectorAll('.tab').forEach(function (t) {
        var isActive = t.dataset.route === active;
        t.classList.toggle('active', isActive);
        t.setAttribute('aria-selected', isActive ? 'true' : 'false');
    });
}

var mainEl = null;

function router() {
    if (!mainEl) mainEl = document.getElementById('main-content');
    abortPending();
    var route = parseRoute();
    updateTabs(route);
    updateHeaderUser();

    var searchInput = document.getElementById('search-input');
    if (route.view !== 'search' && searchInput) searchInput.value = '';

    switch (route.view) {
        case 'dashboard': renderDashboard(mainEl); break;
        case 'repos': renderRepos(mainEl); break;
        case 'repo-detail': renderRepoDetail(mainEl, route.repoId); break;
        case 'file-tree': renderFileTree(mainEl, route.repoId, route.branch, route.path); break;
        case 'blob': renderBlob(mainEl, route.repoId, route.contentHash); break;
        case 'patches': renderPatches(mainEl, route.repoId, route.offset); break;
        case 'search': renderSearch(mainEl, route.query); break;
        case 'users': renderUsers(mainEl); break;
        case 'mirrors': renderMirrors(mainEl); break;
        case 'replication': renderReplication(mainEl); break;
        case 'settings': renderSettings(mainEl); break;
        case 'login': renderLogin(mainEl); break;
        default: renderDashboard(mainEl); break;
    }

    var titles = {
        'dashboard': 'Dashboard',
        'repos': 'Repositories',
        'repo-detail': 'Repository',
        'file-tree': 'Files',
        'blob': 'File',
        'patches': 'Patches',
        'search': 'Search',
        'users': 'Users',
        'mirrors': 'Mirrors',
        'replication': 'Replication',
        'settings': 'Settings',
        'login': 'Login',
    };
    document.title = (titles[route.view] || 'Suture Hub') + ' \u2014 Suture Hub';
}

window.addEventListener('hashchange', router);

function updateHeaderUser() {
    var container = document.getElementById('header-user');
    if (!container) return;
    var session = sessionStorage.getItem('suture-user');
    if (session) {
        try {
            var user = JSON.parse(session);
            container.innerHTML =
                '<span class="user-name">' + escapeHtml(user.username || user.display_name) + '</span>' +
                '<button class="btn btn-secondary btn-sm" id="logout-btn" aria-label="Logout">Logout</button>';
            document.getElementById('logout-btn').addEventListener('click', function () {
                sessionStorage.removeItem('suture-user');
                sessionStorage.removeItem('suture-token');
                updateHeaderUser();
                window.location.hash = '#/';
            });
        } catch (e) {
            sessionStorage.removeItem('suture-user');
            container.innerHTML = '<a href="#/login" class="btn btn-secondary btn-sm">Login</a>';
        }
    } else {
        container.innerHTML = '<a href="#/login" class="btn btn-secondary btn-sm">Login</a>';
    }
}

document.getElementById('search-form').addEventListener('submit', function (e) {
    e.preventDefault();
    var q = document.getElementById('search-input').value.trim();
    if (q) window.location.hash = '#/search?q=' + encodeURIComponent(q);
});

async function checkConnection() {
    var indicator = document.getElementById('connection-indicator');
    try {
        var data = await fetch(API_BASE + '/handshake', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...getHeaders() },
            body: JSON.stringify({ client_version: 1, client_name: 'web-ui' }),
        });
        if (data.ok) {
            var json = await data.json();
            if (json.compatible) {
                indicator.className = 'indicator connected';
                indicator.title = 'Connected';
                indicator.querySelector('.indicator-text').textContent = 'Connected';
            } else {
                indicator.className = 'indicator disconnected';
                indicator.title = 'Incompatible version';
                indicator.querySelector('.indicator-text').textContent = 'Incompatible';
            }
        } else {
            indicator.className = 'indicator disconnected';
            indicator.title = 'Disconnected';
            indicator.querySelector('.indicator-text').textContent = 'Disconnected';
        }
    } catch {
        indicator.className = 'indicator disconnected';
        indicator.title = 'Disconnected';
        indicator.querySelector('.indicator-text').textContent = 'Disconnected';
    }
}

async function renderDashboard(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Dashboard</h2></div>' +
        '<div class="skeleton-wrap" id="dash-skel"><div class="skeleton-cards">' +
        '<div class="skeleton-card"></div><div class="skeleton-card"></div><div class="skeleton-card"></div>' +
        '</div></div>' +
        '<div class="dashboard-cards hidden" id="dash-cards">' +
        '<div class="stat-card"><div class="stat-value" id="stat-repos">\u2014</div><div class="stat-label">Repositories</div></div>' +
        '<div class="stat-card"><div class="stat-value" id="stat-patches">\u2014</div><div class="stat-label">Total Patches</div></div>' +
        '<div class="stat-card"><div class="stat-value" id="stat-users">\u2014</div><div class="stat-label">Users</div></div>' +
        '</div>' +
        '<div class="dashboard-activity hidden" id="dash-activity">' +
        '<h3>Recent Activity</h3><div id="activity-wrap"></div></div>';

    try {
        var reposData = await fetchJSON(API_BASE + '/repos');
        var repoIds = reposData.repo_ids || [];
        var totalPatches = 0;
        var repoInfos = await Promise.all(
            repoIds.map(function (id) {
                return fetchJSON(API_BASE + '/repo/' + encodeURIComponent(id)).catch(function () { return null; });
            })
        );
        var validRepos = [];
        for (var i = 0; i < repoInfos.length; i++) {
            if (repoInfos[i] && repoInfos[i].success) {
                validRepos.push(repoInfos[i]);
                totalPatches += (repoInfos[i].patch_count || 0);
            }
        }

        var usersData = await fetchJSON(API_BASE + '/users');
        var userCount = (usersData.users || []).length;

        hide('dash-skel');
        show('dash-cards');
        document.getElementById('stat-repos').textContent = validRepos.length;
        document.getElementById('stat-patches').textContent = totalPatches;
        document.getElementById('stat-users').textContent = userCount;

        try {
            var actData = await fetchJSON(API_BASE + '/activity');
            var entries = actData.entries || [];
            if (entries.length > 0) {
                show('dash-activity');
                var listHtml = '<ul class="activity-list">';
                entries.slice(0, 20).forEach(function (entry) {
                    listHtml += '<li>' +
                        '<div class="activity-entry">' +
                        '<div class="activity-message">' + escapeHtml(entry.message || entry.action || 'Activity') + '</div>' +
                        '</div>' +
                        '<span class="activity-time">' + formatTimestamp(entry.timestamp || entry.created_at) + '</span>' +
                        '</li>';
                });
                listHtml += '</ul>';
                document.getElementById('activity-wrap').innerHTML = listHtml;
            }
        } catch (e) { }
    } catch (err) {
        hide('dash-skel');
        show('dash-cards');
    }
}

async function renderRepos(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Repositories</h2><span id="repo-count" class="badge"></span>' +
        '<button class="btn btn-primary" id="btn-create-repo">+ New Repository</button></div>' +
        '<div class="form-panel hidden" id="create-repo-form">' +
        '<h3>Create Repository</h3>' +
        '<form id="form-create-repo">' +
        '<label for="new-repo-name">Repository Name</label>' +
        '<input type="text" id="new-repo-name" placeholder="my-project" required autocomplete="off">' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-create-repo">Cancel</button>' +
        '</div></form></div>' +
        '<div id="repos-loading" class="loading">Loading repositories&hellip;</div>' +
        '<div id="repos-error" class="error-banner hidden"></div>' +
        '<div id="repos-table-wrap" class="table-wrap hidden">' +
        '<table><thead><tr><th>Name</th><th>Patches</th><th>Branches</th></tr></thead>' +
        '<tbody id="repos-tbody"></tbody></table></div>' +
        '<div id="repos-empty" class="empty-state hidden">No repositories found.</div>';

    container.querySelector('#btn-create-repo').addEventListener('click', function () { togglePanel('create-repo-form'); });
    container.querySelector('#cancel-create-repo').addEventListener('click', function () { hide('create-repo-form'); });
    container.querySelector('#form-create-repo').addEventListener('submit', async function (e) {
        e.preventDefault();
        var name = document.getElementById('new-repo-name').value.trim();
        if (!name) return;
        try {
            await fetchJSON(API_BASE + '/repos', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ repo_id: name }),
            });
            toast('Repository "' + name + '" created', 'success');
            document.getElementById('form-create-repo').reset();
            hide('create-repo-form');
            loadReposData();
        } catch (err) {
            toast('Failed to create repository: ' + err.message, 'error');
        }
    });

    await loadReposData();
}

async function loadReposData() {
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
                    return { repo_id: id, patch_count: 0, branches: [], success: true };
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
                '<td><a href="#/repo/' + encodeURIComponent(repo.repo_id) + '" class="file-link">' + escapeHtml(repo.repo_id) + '</a></td>' +
                '<td>' + repo.patch_count + '</td>' +
                '<td>' + repo.branches.length + '</td>';
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

async function renderRepoDetail(container, repoId) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a>' +
        ' <span class="separator">/</span> ' +
        '<span class="current">' + escapeHtml(repoId) + '</span></div>' +
        '<div id="repo-detail-loading" class="loading">Loading repository&hellip;</div>' +
        '<div id="repo-detail-error" class="error-banner hidden"></div>' +
        '<div id="repo-detail-content" class="hidden"></div>';

    try {
        var info = await fetchJSON(API_BASE + '/repo/' + encodeURIComponent(repoId));
        if (!info.success) throw new Error('Repository not found');

        hide('repo-detail-loading');

        var branchesHtml = '';
        (info.branches || []).forEach(function (b) {
            var prot = b.protected ? 'protected' : 'unprotected';
            var label = b.protected ? '\uD83D\uDD12' : '\uD83D\uDD13';
            branchesHtml +=
                '<div style="display:flex;align-items:center;gap:0.3rem;margin:0.2rem 0;flex-wrap:wrap">' +
                '<a href="#/repo/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(b.name) + '" class="branch-tag">' + escapeHtml(b.name) + '</a>' +
                '<span class="protection-badge ' + prot + '" data-repo="' + escapeHtml(repoId) + '" data-branch="' + escapeHtml(b.name) + '" title="Toggle protection" role="button" tabindex="0" aria-label="Toggle protection for ' + escapeHtml(b.name) + '">' + label + '</span>' +
                '<button class="btn btn-danger btn-sm branch-delete" data-repo="' + escapeHtml(repoId) + '" data-branch="' + escapeHtml(b.name) + '" aria-label="Delete branch ' + escapeHtml(b.name) + '">\u00D7</button>' +
                '</div>';
        });

        if (!branchesHtml) branchesHtml = '<span style="color:var(--text-secondary)">No branches</span>';

        var patchesHtml = '';
        try {
            var patchesData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/patches?limit=10');
            var patches = patchesData.patches || [];
            if (patches.length > 0) {
                patches.forEach(function (p) {
                    var type = (p.operation_type || 'unknown').toLowerCase();
                    var action = type === 'create' ? 'created' : type === 'modify' ? 'modified' : type === 'delete' ? 'deleted' : type;
                    patchesHtml +=
                        '<tr>' +
                        '<td class="mono hash-truncate">' + escapeHtml(p.id || '') + '</td>' +
                        '<td><span class="patch-type-badge ' + type + '">' + escapeHtml(type) + '</span></td>' +
                        '<td class="mono">' + escapeHtml(p.target_path || '') + '</td>' +
                        '<td>' + escapeHtml(p.author || '') + '</td>' +
                        '<td>' + escapeHtml(p.message || '') + '</td>' +
                        '<td class="mono">' + formatTimestamp(p.timestamp) + '</td>' +
                        '</tr>';
                });
                patchesHtml =
                    '<div class="table-wrap"><table><thead><tr>' +
                    '<th>ID</th><th>Type</th><th>Path</th><th>Author</th><th>Message</th><th>Time</th>' +
                    '</tr></thead><tbody>' + patchesHtml + '</tbody></table></div>';
            } else {
                patchesHtml = '<span style="color:var(--text-secondary)">No patches</span>';
            }
        } catch (e) {
            patchesHtml = '<span style="color:var(--text-secondary)">Could not load patches</span>';
        }

        var contentEl = document.getElementById('repo-detail-content');
        contentEl.innerHTML =
            '<div class="repo-header">' +
            '<div><h2>' + escapeHtml(repoId) + '</h2>' +
            '<div class="repo-meta">' +
            '<span>' + info.patch_count + ' patches</span>' +
            '<span>' + (info.branches || []).length + ' branches</span>' +
            '</div></div>' +
            '<button class="btn btn-danger" id="btn-delete-repo">Delete Repo</button>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Branches <button class="btn btn-secondary btn-sm" id="btn-toggle-create-branch">+ New Branch</button></h3>' +
            '<div class="form-panel hidden" id="create-branch-form">' +
            '<form id="form-create-branch">' +
            '<label for="new-branch-name">Branch Name</label>' +
            '<input type="text" id="new-branch-name" placeholder="main" required autocomplete="off">' +
            '<label for="new-branch-target">Target (commit ID)</label>' +
            '<input type="text" id="new-branch-target" placeholder="commit-hash" autocomplete="off">' +
            '<div class="form-actions">' +
            '<button type="submit" class="btn btn-primary">Create</button>' +
            '<button type="button" class="btn btn-secondary" id="cancel-create-branch">Cancel</button>' +
            '</div></form></div>' +
            '<div id="branches-wrap">' + branchesHtml + '</div>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Recent Patches <a href="#/repo/' + encodeURIComponent(repoId) + '/patches" class="btn btn-secondary btn-sm">View all</a></h3>' +
            patchesHtml +
            '</div>';

        show('repo-detail-content');

        contentEl.querySelector('#btn-delete-repo').addEventListener('click', async function () {
            var ok = await confirmDialog('Delete Repository', 'Are you sure you want to delete "' + repoId + '"? This cannot be undone.');
            if (!ok) return;
            try {
                await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId), { method: 'DELETE' });
                toast('Repository "' + repoId + '" deleted', 'success');
                window.location.hash = '#/repos';
            } catch (err) {
                toast('Failed to delete repository: ' + err.message, 'error');
            }
        });

        contentEl.querySelector('#btn-toggle-create-branch').addEventListener('click', function () { togglePanel('create-branch-form'); });
        contentEl.querySelector('#cancel-create-branch').addEventListener('click', function () { hide('create-branch-form'); });
        contentEl.querySelector('#form-create-branch').addEventListener('submit', async function (e) {
            e.preventDefault();
            var name = document.getElementById('new-branch-name').value.trim();
            var target = document.getElementById('new-branch-target').value.trim();
            if (!name) return;
            try {
                await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/branches', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: name, target: target || '' }),
                });
                toast('Branch "' + name + '" created', 'success');
                document.getElementById('form-create-branch').reset();
                hide('create-branch-form');
                router();
            } catch (err) {
                toast('Failed to create branch: ' + err.message, 'error');
            }
        });

        contentEl.querySelector('#branches-wrap').addEventListener('click', async function (e) {
            var badge = e.target.closest('.protection-badge');
            if (badge) {
                var br = badge.dataset.branch;
                var rId = badge.dataset.repo;
                var isProt = badge.classList.contains('protected');
                var action = isProt ? 'unprotect' : 'protect';
                try {
                    await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(rId) + '/' + action + '/' + encodeURIComponent(br), { method: 'POST' });
                    toast('Branch "' + br + '" ' + (isProt ? 'unprotected' : 'protected'), 'success');
                    badge.classList.toggle('protected', !isProt);
                    badge.classList.toggle('unprotected', isProt);
                    badge.textContent = isProt ? '\uD83D\uDD13' : '\uD83D\uDD12';
                } catch (err) {
                    toast('Failed to toggle protection: ' + err.message, 'error');
                }
                return;
            }

            var delBtn = e.target.closest('.branch-delete');
            if (delBtn) {
                var br = delBtn.dataset.branch;
                var ok = await confirmDialog('Delete Branch', 'Delete branch "' + br + '"?');
                if (!ok) return;
                try {
                    await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/branches/' + encodeURIComponent(br), { method: 'DELETE' });
                    toast('Branch "' + br + '" deleted', 'success');
                    router();
                } catch (err) {
                    toast('Failed to delete branch: ' + err.message, 'error');
                }
            }
        });
    } catch (err) {
        hide('repo-detail-loading');
        var errEl = document.getElementById('repo-detail-error');
        errEl.textContent = 'Failed to load repository: ' + err.message;
        show('repo-detail-error');
    }
}

async function renderFileTree(container, repoId, branch, path) {
    container.innerHTML =
        '<div class="breadcrumb" id="tree-breadcrumb"></div>' +
        '<div id="tree-loading" class="loading">Loading file tree&hellip;</div>' +
        '<div id="tree-error" class="error-banner hidden"></div>' +
        '<div id="tree-content" class="hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(branch));
        if (!data.success) throw new Error('Failed to load tree');
        var files = data.files || [];

        hide('tree-loading');

        var parts = path ? path.split('/') : [];
        var bc = document.getElementById('tree-breadcrumb');
        var bcHtml =
            '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(branch) + '">' + escapeHtml(branch) + '</a>';
        var cumPath = '';
        parts.forEach(function (part, i) {
            cumPath += (i > 0 ? '/' : '') + part;
            bcHtml += ' <span class="separator">/</span> ';
            if (i === parts.length - 1) {
                bcHtml += '<span class="current">' + escapeHtml(part) + '</span>';
            } else {
                bcHtml += '<a href="#/repo/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(branch) + '?path=' + encodeURIComponent(cumPath) + '">' + escapeHtml(part) + '</a>';
            }
        });
        bc.innerHTML = bcHtml;

        var prefix = path ? path + '/' : '';
        var dirs = {};
        var currentFiles = [];

        files.forEach(function (f) {
            var fp = f.path;
            if (!fp.startsWith(prefix)) return;
            var rest = fp.slice(prefix.length);
            if (!rest) return;
            var slashIdx = rest.indexOf('/');
            if (slashIdx === -1) {
                currentFiles.push(f);
            } else {
                var dirName = rest.slice(0, slashIdx);
                if (!dirs[dirName]) dirs[dirName] = [];
                dirs[dirName].push(f);
            }
        });

        var dirNames = Object.keys(dirs).sort();
        currentFiles.sort(function (a, b) { return a.path.localeCompare(b.path); });

        if (dirNames.length === 0 && currentFiles.length === 0) {
            document.getElementById('tree-content').innerHTML = '<div class="empty-state">Empty directory.</div>';
            show('tree-content');
            return;
        }

        var tableHtml = '<div class="table-wrap"><table class="file-tree-table"><thead><tr>' +
            '<th>Name</th><th>Type</th><th>Hash</th>' +
            '</tr></thead><tbody>';

        if (path) {
            var parentPath = parts.length > 1 ? parts.slice(0, -1).join('/') : '';
            var parentHref = '#/repo/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(branch);
            if (parentPath) parentHref += '?path=' + encodeURIComponent(parentPath);
            tableHtml += '<tr><td><a class="dir-link file-name" href="' + parentHref + '"><span class="file-icon">\u{1F5C1}</span> ..</a></td><td>directory</td><td></td></tr>';
        }

        dirNames.forEach(function (name) {
            var dirPath = path ? path + '/' + name : name;
            var count = dirs[name].length;
            tableHtml +=
                '<tr><td><a class="dir-link file-name" href="#/repo/' + encodeURIComponent(repoId) + '/tree/' + encodeURIComponent(branch) + '?path=' + encodeURIComponent(dirPath) + '">' +
                '<span class="file-icon">\uD83D\uDCC1</span> ' + escapeHtml(name) + '</a></td>' +
                '<td>directory</td>' +
                '<td class="mono" style="color:var(--text-secondary)">' + count + ' files</td></tr>';
        });

        currentFiles.forEach(function (f) {
            var name = f.path.slice(prefix.length);
            tableHtml +=
                '<tr><td><a class="file-link file-name" href="#/repo/' + encodeURIComponent(repoId) + '/blob/' + encodeURIComponent(f.content_hash) + '">' +
                '<span class="file-icon">\uD83D\uDCC4</span> ' + escapeHtml(name) + '</a></td>' +
                '<td>file</td>' +
                '<td class="mono hash-truncate">' + escapeHtml(f.content_hash) + '</td></tr>';
        });

        tableHtml += '</tbody></table></div>';
        document.getElementById('tree-content').innerHTML = tableHtml;
        show('tree-content');
    } catch (err) {
        hide('tree-loading');
        var errEl = document.getElementById('tree-error');
        errEl.textContent = 'Failed to load file tree: ' + err.message;
        show('tree-error');
    }
}

async function renderBlob(container, repoId, contentHash) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Blob</span></div>' +
        '<div id="blob-loading" class="loading">Loading file&hellip;</div>' +
        '<div id="blob-error" class="error-banner hidden"></div>' +
        '<div id="blob-content" class="hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/blobs/' + encodeURIComponent(contentHash));
        if (!data.success) throw new Error('Blob not found');

        hide('blob-loading');

        var raw = atob(data.data || '');
        var size = raw.length;
        var blobEl = document.getElementById('blob-content');

        if (isBinary(raw)) {
            blobEl.innerHTML =
                '<div class="blob-container">' +
                '<div class="blob-header">' +
                '<span class="blob-hash">' + escapeHtml(contentHash) + '</span>' +
                '<span class="blob-size">' + formatBytes(size) + '</span>' +
                '</div>' +
                '<div class="blob-binary">' +
                '<span class="blob-binary-icon">\uD83D\uDCC4</span>' +
                'Binary file (' + formatBytes(size) + ')' +
                '</div></div>';
        } else {
            blobEl.innerHTML =
                '<div class="blob-container">' +
                '<div class="blob-header">' +
                '<span class="blob-hash">' + escapeHtml(contentHash) + '</span>' +
                '<span class="blob-size">' + formatBytes(size) + '</span>' +
                '</div>' +
                '<div class="blob-content"><pre><code>' + escapeHtml(raw) + '</code></pre></div>' +
                '</div>';
        }

        show('blob-content');
    } catch (err) {
        hide('blob-loading');
        var errEl = document.getElementById('blob-error');
        errEl.textContent = 'Failed to load file: ' + err.message;
        show('blob-error');
    }
}

async function renderPatches(container, repoId, offset) {
    offset = offset || 0;
    var limit = 50;

    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Patches</span></div>' +
        '<div id="patches-loading" class="loading">Loading patches&hellip;</div>' +
        '<div id="patches-error" class="error-banner hidden"></div>' +
        '<div id="patches-content" class="hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/patches?offset=' + offset + '&limit=' + limit);
        var patches = data.patches || [];

        hide('patches-loading');

        if (patches.length === 0 && offset === 0) {
            document.getElementById('patches-content').innerHTML = '<div class="empty-state">No patches found.</div>';
            show('patches-content');
            return;
        }

        var html = '<div class="table-wrap"><table><thead><tr>' +
            '<th>ID</th><th>Type</th><th>Path</th><th>Author</th><th>Message</th><th>Timestamp</th>' +
            '</tr></thead><tbody>';

        patches.forEach(function (p) {
            var type = (p.operation_type || 'unknown').toLowerCase();
            html +=
                '<tr>' +
                '<td class="mono hash-truncate">' + escapeHtml(p.id || '') + '</td>' +
                '<td><span class="patch-type-badge ' + type + '">' + escapeHtml(type) + '</span></td>' +
                '<td class="mono">' + escapeHtml(p.target_path || '') + '</td>' +
                '<td>' + escapeHtml(p.author || '') + '</td>' +
                '<td>' + escapeHtml(p.message || '') + '</td>' +
                '<td class="mono">' + formatTimestamp(p.timestamp) + '</td>' +
                '</tr>';
        });

        html += '</tbody></table></div>';

        html += '<div class="pagination">';
        if (offset > 0) {
            html += '<a href="#/repo/' + encodeURIComponent(repoId) + '/patches?offset=' + (offset - limit) + '" class="btn btn-secondary btn-sm">&larr; Previous</a>';
        }
        html += '<span class="page-info">' + (offset + 1) + '\u2013' + (offset + patches.length) + '</span>';
        if (patches.length === limit) {
            html += '<a href="#/repo/' + encodeURIComponent(repoId) + '/patches?offset=' + (offset + limit) + '" class="btn btn-secondary btn-sm">Next &rarr;</a>';
        }
        html += '</div>';

        document.getElementById('patches-content').innerHTML = html;
        show('patches-content');
    } catch (err) {
        hide('patches-loading');
        var errEl = document.getElementById('patches-error');
        errEl.textContent = 'Failed to load patches: ' + err.message;
        show('patches-error');
    }
}

async function renderSearch(container, query) {
    document.getElementById('search-input').value = query || '';

    if (!query) {
        container.innerHTML =
            '<div class="section-header"><h2>Search</h2></div>' +
            '<div class="search-empty">Enter a query in the search bar above.</div>';
        return;
    }

    container.innerHTML =
        '<div class="section-header"><h2>Search Results for "' + escapeHtml(query) + '"</h2></div>' +
        '<div id="search-loading" class="loading">Searching&hellip;</div>' +
        '<div id="search-error" class="error-banner hidden"></div>' +
        '<div id="search-results" class="search-results hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/search?q=' + encodeURIComponent(query));
        hide('search-loading');

        var repos = data.repos || [];
        var patches = data.patches || [];

        if (repos.length === 0 && patches.length === 0) {
            document.getElementById('search-results').innerHTML = '<div class="search-empty">No results found.</div>';
            show('search-results');
            return;
        }

        var html = '';

        if (repos.length > 0) {
            html += '<h3>Repositories</h3>';
            repos.forEach(function (r) {
                var name = r.repo_id || r.name || '';
                html += '<div class="search-item">' +
                    '<a href="#/repo/' + encodeURIComponent(name) + '">' + escapeHtml(name) + '</a>' +
                    '<div class="search-item-meta">' + (r.patch_count || 0) + ' patches</div>' +
                    '</div>';
            });
        }

        if (patches.length > 0) {
            html += '<h3>Patches</h3>';
            patches.forEach(function (p) {
                var repoName = p.repo_id || '';
                var patchId = p.id || '';
                html += '<div class="search-item">' +
                    '<a href="#/repo/' + encodeURIComponent(repoName) + '">' + escapeHtml(patchId) + '</a>' +
                    '<div class="search-item-meta">' +
                    escapeHtml(p.message || p.target_path || '') +
                    (repoName ? ' \u2014 ' + escapeHtml(repoName) : '') +
                    '</div></div>';
            });
        }

        document.getElementById('search-results').innerHTML = html;
        show('search-results');
    } catch (err) {
        hide('search-loading');
        var errEl = document.getElementById('search-error');
        errEl.textContent = 'Search failed: ' + err.message;
        show('search-error');
    }
}

async function renderUsers(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Users</h2><span id="user-count" class="badge"></span>' +
        '<button class="btn btn-primary" id="btn-toggle-create-user">+ New User</button></div>' +
        '<div class="form-panel hidden" id="create-user-form">' +
        '<h3>Create User</h3>' +
        '<form id="form-create-user">' +
        '<label for="new-user-username">Username</label>' +
        '<input type="text" id="new-user-username" placeholder="username" required autocomplete="off">' +
        '<label for="new-user-display">Display Name</label>' +
        '<input type="text" id="new-user-display" placeholder="Display Name" autocomplete="off">' +
        '<p class="form-note">An API token will be generated automatically upon creation.</p>' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-create-user">Cancel</button>' +
        '</div></form></div>' +
        '<div id="users-loading" class="loading">Loading users&hellip;</div>' +
        '<div id="users-error" class="error-banner hidden"></div>' +
        '<div id="users-table-wrap" class="table-wrap hidden">' +
        '<table><thead><tr><th>Username</th><th>Display Name</th><th>Role</th><th>Created</th><th>Actions</th></tr></thead>' +
        '<tbody id="users-tbody"></tbody></table></div>' +
        '<div id="users-empty" class="empty-state hidden">No users found.</div>';

    container.querySelector('#btn-toggle-create-user').addEventListener('click', function () { togglePanel('create-user-form'); });
    container.querySelector('#cancel-create-user').addEventListener('click', function () { hide('create-user-form'); });
    container.querySelector('#form-create-user').addEventListener('submit', async function (e) {
        e.preventDefault();
        var username = document.getElementById('new-user-username').value.trim();
        var displayName = document.getElementById('new-user-display').value.trim();
        if (!username) return;
        try {
            await fetchJSON(API_BASE + '/auth/register', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ username: username, display_name: displayName || username }),
            });
            toast('User "' + username + '" created', 'success');
            document.getElementById('form-create-user').reset();
            hide('create-user-form');
            loadUsersData();
        } catch (err) {
            toast('Failed to create user: ' + err.message, 'error');
        }
    });

    container.querySelector('#users-tbody').addEventListener('change', async function (e) {
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
            loadUsersData();
        }
    });

    container.querySelector('#users-tbody').addEventListener('click', async function (e) {
        var btn = e.target.closest('.user-delete');
        if (!btn) return;
        var username = btn.dataset.user;
        var ok = await confirmDialog('Delete User', 'Are you sure you want to delete user "' + username + '"? This cannot be undone.');
        if (!ok) return;
        try {
            await fetchJSON(API_BASE + '/users/' + encodeURIComponent(username), { method: 'DELETE' });
            toast('User "' + username + '" deleted', 'success');
            loadUsersData();
        } catch (err) {
            toast('Failed to delete user: ' + err.message, 'error');
        }
    });

    await loadUsersData();
}

async function loadUsersData() {
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
            var role = (user.role || 'member').toLowerCase();
            var tr = document.createElement('tr');
            tr.innerHTML =
                '<td class="mono">' + escapeHtml(user.username) + '</td>' +
                '<td>' + escapeHtml(user.display_name) + '</td>' +
                '<td><select class="role-select" data-user="' + escapeHtml(user.username) + '">' +
                '<option value="admin"' + (role === 'admin' ? ' selected' : '') + '>admin</option>' +
                '<option value="member"' + (role === 'member' ? ' selected' : '') + '>member</option>' +
                '<option value="reader"' + (role === 'reader' ? ' selected' : '') + '>reader</option>' +
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

async function renderMirrors(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Mirrors</h2><span id="mirror-count" class="badge"></span>' +
        '<button class="btn btn-primary" id="btn-toggle-add-mirror">+ Add Mirror</button></div>' +
        '<div class="form-panel hidden" id="add-mirror-form">' +
        '<h3>Add Mirror</h3>' +
        '<form id="form-add-mirror">' +
        '<label for="mirror-remote-url">Remote URL</label>' +
        '<input type="text" id="mirror-remote-url" placeholder="https://example.com/repo" required autocomplete="off">' +
        '<label for="mirror-local-repo">Local Repository</label>' +
        '<input type="text" id="mirror-local-repo" placeholder="my-project" required autocomplete="off">' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Add Mirror</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-add-mirror">Cancel</button>' +
        '</div></form></div>' +
        '<div id="mirrors-loading" class="loading">Loading mirrors&hellip;</div>' +
        '<div id="mirrors-error" class="error-banner hidden"></div>' +
        '<div id="mirrors-table-wrap" class="table-wrap hidden">' +
        '<table><thead><tr><th>Local Repo</th><th>Remote URL</th><th>Last Sync</th><th>Actions</th></tr></thead>' +
        '<tbody id="mirrors-tbody"></tbody></table></div>' +
        '<div id="mirrors-empty" class="empty-state hidden">No mirrors configured.</div>';

    container.querySelector('#btn-toggle-add-mirror').addEventListener('click', function () { togglePanel('add-mirror-form'); });
    container.querySelector('#cancel-add-mirror').addEventListener('click', function () { hide('add-mirror-form'); });
    container.querySelector('#form-add-mirror').addEventListener('submit', async function (e) {
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
            loadMirrorsData();
        } catch (err) {
            toast('Failed to add mirror: ' + err.message, 'error');
        }
    });

    container.querySelector('#mirrors-tbody').addEventListener('click', async function (e) {
        var btn = e.target.closest('.mirror-sync');
        if (!btn) return;
        var id = btn.dataset.id;
        var repo = btn.dataset.repo;
        btn.disabled = true;
        btn.textContent = 'Syncing...';
        try {
            await fetchJSON(API_BASE + '/mirror/sync', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ mirror_id: Number(id) }),
            });
            toast('Sync completed for "' + repo + '"', 'success');
            loadMirrorsData();
        } catch (err) {
            toast('Sync failed: ' + err.message, 'error');
            btn.disabled = false;
            btn.textContent = 'Sync';
        }
    });

    await loadMirrorsData();
}

async function loadMirrorsData() {
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
                '<button class="btn btn-primary btn-sm mirror-sync" data-id="' + (m.id || '') +
                '" data-repo="' + escapeHtml(m.local_repo || m.repo_id || '') +
                '">Sync</button>' +
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

async function renderReplication(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Replication</h2><span id="replication-role-badge" class="badge"></span>' +
        '<button class="btn btn-primary" id="btn-toggle-add-peer">+ Add Peer</button></div>' +
        '<div class="form-panel hidden" id="add-peer-form">' +
        '<h3>Add Peer</h3>' +
        '<form id="form-add-peer">' +
        '<label for="peer-url">Peer URL</label>' +
        '<input type="text" id="peer-url" placeholder="https://hub.example.com" required autocomplete="off">' +
        '<label for="peer-role">Role</label>' +
        '<select id="peer-role">' +
        '<option value="follower">Follower</option>' +
        '<option value="leader">Leader</option>' +
        '</select>' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Add Peer</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-add-peer">Cancel</button>' +
        '</div></form></div>' +
        '<div id="replication-loading" class="loading">Loading replication status&hellip;</div>' +
        '<div id="replication-error" class="error-banner hidden"></div>' +
        '<div id="replication-info-wrap" class="hidden">' +
        '<div class="replication-summary">' +
        '<div><div class="detail-label">Log Sequence</div><div id="replication-current-seq" class="detail-value">\u2014</div></div>' +
        '<div><div class="detail-label">Peers</div><div id="replication-peer-count" class="detail-value">\u2014</div></div>' +
        '</div>' +
        '<div class="table-wrap"><table id="replication-table"><thead><tr>' +
        '<th>Peer URL</th><th>Role</th><th>Status</th><th>Sync Seq</th><th>Actions</th>' +
        '</tr></thead><tbody id="replication-tbody"></tbody></table></div></div>' +
        '<div id="replication-empty" class="empty-state hidden">No replication peers configured. This hub is running in standalone mode.</div>';

    container.querySelector('#btn-toggle-add-peer').addEventListener('click', function () { togglePanel('add-peer-form'); });
    container.querySelector('#cancel-add-peer').addEventListener('click', function () { hide('add-peer-form'); });
    container.querySelector('#form-add-peer').addEventListener('submit', async function (e) {
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
            loadReplicationData();
        } catch (err) {
            toast('Failed to add peer: ' + err.message, 'error');
        }
    });

    container.querySelector('#replication-tbody').addEventListener('click', async function (e) {
        var btn = e.target.closest('.peer-remove');
        if (!btn) return;
        var id = btn.dataset.id;
        var url = btn.dataset.url;
        var ok = await confirmDialog('Remove Peer', 'Are you sure you want to remove peer "' + url + '"?');
        if (!ok) return;
        try {
            await fetchJSON(API_BASE + '/replication/peers/' + encodeURIComponent(id), { method: 'DELETE' });
            toast('Peer removed', 'success');
            loadReplicationData();
        } catch (err) {
            toast('Failed to remove peer: ' + err.message, 'error');
        }
    });

    await loadReplicationData();
}

async function loadReplicationData() {
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
                var peerStatus = (peer.status || 'active').toLowerCase();
                var tr = document.createElement('tr');
                tr.innerHTML =
                    '<td class="mono">' + escapeHtml(peer.peer_url) + '</td>' +
                    '<td><span class="role-badge ' + escapeHtml(peer.role) + '">' + escapeHtml(peer.role) + '</span></td>' +
                    '<td><span class="status-badge ' + peerStatus + '">' + escapeHtml(peerStatus) + '</span></td>' +
                    '<td class="mono">' + (peer.last_sync_seq || '\u2014') + '</td>' +
                    '<td class="actions-cell">' +
                    '<button class="btn btn-danger btn-sm peer-remove" data-id="' + (peer.id || '') + '" data-url="' + escapeHtml(peer.peer_url) + '">Remove</button>' +
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

async function renderSettings(container) {
    container.innerHTML =
        '<div class="section-header"><h2>Settings</h2></div>' +
        '<div id="settings-loading" class="settings-panel">' +
        '<div class="skeleton-wrap"><div class="skeleton-card" style="height:60px;margin-bottom:1rem"></div>' +
        '<div class="skeleton-card" style="height:60px;margin-bottom:1rem"></div>' +
        '<div class="skeleton-card" style="height:60px"></div></div></div>' +
        '<div class="settings-panel hidden" id="settings-content">' +
        '<div class="settings-row"><div class="settings-key">Hub Version</div><div class="settings-value" id="setting-version">\u2014</div></div>' +
        '<div class="settings-row"><div class="settings-key">Replication Role</div><div class="settings-value" id="setting-replication-role">\u2014</div></div>' +
        '<div class="settings-row"><div class="settings-key">Auth Mode</div><div class="settings-value" id="setting-auth-mode">\u2014</div></div>' +
        '</div>';

    try {
        var handshake = await fetchJSON(API_BASE + '/handshake', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ client_version: 1, client_name: 'web-ui' }),
        });

        var replData = null;
        try {
            replData = await fetchJSON(API_BASE + '/replication/status');
        } catch (e) { }

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
        document.getElementById('setting-replication-role').textContent = '\u2014';
        document.getElementById('setting-auth-mode').textContent = '\u2014';
        show('settings-content');
    }
}

async function renderLogin(container) {
    if (sessionStorage.getItem('suture-user')) {
        window.location.hash = '#/';
        return;
    }

    container.innerHTML =
        '<div class="login-container">' +
        '<div class="login-card">' +
        '<h2>Sign in to Suture Hub</h2>' +
        '<form id="form-login">' +
        '<label for="login-username">Username</label>' +
        '<input type="text" id="login-username" placeholder="username" required autocomplete="username">' +
        '<label for="login-token">API Token</label>' +
        '<input type="password" id="login-token" placeholder="your-api-token" required autocomplete="current-password">' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Login</button>' +
        '</div></form>' +
        '<div id="login-error" class="login-error hidden"></div>' +
        '</div></div>';

    container.querySelector('#form-login').addEventListener('submit', async function (e) {
        e.preventDefault();
        var username = document.getElementById('login-username').value.trim();
        var token = document.getElementById('login-token').value.trim();
        if (!username || !token) return;

        var errEl = document.getElementById('login-error');
        hide('login-error');

        try {
            var data = await fetchJSON(API_BASE + '/auth/login', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ username: username, token: token }),
            });

            if (data.success) {
                sessionStorage.setItem('suture-token', data.token || token);
                sessionStorage.setItem('suture-user', JSON.stringify(data.user || { username: username }));
                updateHeaderUser();
                window.location.hash = '#/';
            } else {
                errEl.textContent = data.error || 'Login failed';
                show('login-error');
            }
        } catch (err) {
            errEl.textContent = err.message;
            show('login-error');
        }
    });
}

checkConnection();
setInterval(checkConnection, 30000);
router();
