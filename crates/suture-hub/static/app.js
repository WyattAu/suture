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

    m = hash.match(/^\/repo\/([^/]+)\/issues$/);
    if (m) return { view: 'issues', repoId: decodeURIComponent(m[1]) };

    m = hash.match(/^\/issue\/(\d+)$/);
    if (m) return { view: 'issue-detail', issueId: parseInt(m[1]) };

    m = hash.match(/^\/repo\/([^/]+)\/pulls$/);
    if (m) return { view: 'pulls', repoId: decodeURIComponent(m[1]) };

    m = hash.match(/^\/repo\/([^/]+)\/wiki(?:\/([^?]+))?(?:\?(.*))?$/);
    if (m) return { view: 'wiki', repoId: decodeURIComponent(m[1]), pageTitle: m[2] ? decodeURIComponent(m[2]) : null };

    m = hash.match(/^\/repo\/([^/]+)\/releases$/);
    if (m) return { view: 'releases', repoId: decodeURIComponent(m[1]) };

    m = hash.match(/^\/pull\/(\d+)$/);
    if (m) return { view: 'pull-detail', pullId: parseInt(m[1]) };

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
    if (v === 'repos' || v === 'repo-detail' || v === 'file-tree' || v === 'blob' || v === 'patches' || v === 'issues' || v === 'issue-detail' || v === 'pulls' || v === 'pull-detail' || v === 'wiki' || v === 'releases') return 'repos';
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
        case 'issues': renderIssues(mainEl, route.repoId); break;
        case 'issue-detail': renderIssueDetail(mainEl, route.issueId); break;
        case 'pulls': renderPulls(mainEl, route.repoId); break;
        case 'pull-detail': renderPullDetail(mainEl, route.pullId); break;
        case 'wiki': renderWiki(mainEl, route.repoId, route.pageTitle); break;
        case 'releases': renderReleases(mainEl, route.repoId); break;
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
        'issues': 'Issues',
        'issue-detail': 'Issue',
        'pulls': 'Pull Requests',
        'pull-detail': 'Pull Request',
        'wiki': 'Wiki',
        'releases': 'Releases',
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
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Issues <a href="#/repo/' + encodeURIComponent(repoId) + '/issues" class="btn btn-secondary btn-sm">View all</a></h3>' +
            '<div id="repo-issues-loading" class="loading">Loading issues&hellip;</div>' +
            '<div id="repo-issues-content" class="hidden"></div>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Pull Requests <a href="#/repo/' + encodeURIComponent(repoId) + '/pulls" class="btn btn-secondary btn-sm">View all</a></h3>' +
            '<div id="repo-pulls-loading" class="loading">Loading pull requests&hellip;</div>' +
            '<div id="repo-pulls-content" class="hidden"></div>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Visibility</h3>' +
            '<div id="repo-visibility-row" style="display:flex;align-items:center;gap:0.5rem">' +
            '<span id="repo-visibility-label" style="color:var(--text-secondary)">Loading&hellip;</span>' +
            '<button class="btn btn-secondary btn-sm" id="btn-toggle-visibility">Toggle</button>' +
            '</div></div>' +

            '<div class="repo-section">' +
            '<h3>Fork</h3>' +
            '<form id="form-fork" style="display:flex;gap:0.5rem;align-items:end">' +
            '<div><label for="fork-target">Target Repo ID</label><input type="text" id="fork-target" placeholder="new-repo-name" required autocomplete="off"></div>' +
            '<button type="submit" class="btn btn-primary">Fork</button></form>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Wiki <a href="#/repo/' + encodeURIComponent(repoId) + '/wiki" class="btn btn-secondary btn-sm">View all</a></h3>' +
            '<div id="repo-wiki-loading" class="loading">Loading wiki&hellip;</div>' +
            '<div id="repo-wiki-content" class="hidden"></div>' +
            '</div>' +

            '<div class="repo-section">' +
            '<h3>Releases <a href="#/repo/' + encodeURIComponent(repoId) + '/releases" class="btn btn-secondary btn-sm">View all</a></h3>' +
            '<div id="repo-releases-loading" class="loading">Loading releases&hellip;</div>' +
            '<div id="repo-releases-content" class="hidden"></div>' +
            '</div>';

        show('repo-detail-content');

        (async function () {
            try {
                var issuesData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/issues');
                var issues = issuesData.issues || [];
                var el = document.getElementById('repo-issues-content');
                hide('repo-issues-loading');
                if (issues.length === 0) {
                    el.innerHTML = '<span style="color:var(--text-secondary)">No issues</span>';
                } else {
                    var html = '<div class="table-wrap"><table><thead><tr><th>#</th><th>Title</th><th>Status</th><th>Author</th><th>Updated</th></tr></thead><tbody>';
                    issues.slice(0, 5).forEach(function (issue) {
                        var statusClass = issue.status === 'open' ? 'status-open' : 'status-closed';
                        html += '<tr>' +
                            '<td><a href="#/issue/' + issue.id + '" class="file-link">' + issue.id + '</a></td>' +
                            '<td><a href="#/issue/' + issue.id + '">' + escapeHtml(issue.title) + '</a></td>' +
                            '<td><span class="issue-status-badge ' + statusClass + '">' + escapeHtml(issue.status) + '</span></td>' +
                            '<td>' + escapeHtml(issue.author) + '</td>' +
                            '<td>' + formatTimestamp(issue.updated_at) + '</td>' +
                            '</tr>';
                    });
                    html += '</tbody></table></div>';
                    el.innerHTML = html;
                }
                show('repo-issues-content');
            } catch (e) {
                var el = document.getElementById('repo-issues-content');
                if (el) el.innerHTML = '<span style="color:var(--text-secondary)">Could not load issues</span>';
                hide('repo-issues-loading');
                show('repo-issues-content');
            }
        })();

        (async function () {
            try {
                var pullsData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/pulls');
                var pulls = pullsData.pulls || [];
                var el = document.getElementById('repo-pulls-content');
                hide('repo-pulls-loading');
                if (pulls.length === 0) {
                    el.innerHTML = '<span style="color:var(--text-secondary)">No pull requests</span>';
                } else {
                    var html = '<div class="table-wrap"><table><thead><tr><th>#</th><th>Title</th><th>Status</th><th>Branch</th><th>Author</th></tr></thead><tbody>';
                    pulls.slice(0, 5).forEach(function (pr) {
                        var sc = pr.status === 'open' ? 'status-open' : pr.status === 'merged' ? 'pr-status-merged' : 'status-closed';
                        html += '<tr>' +
                            '<td><a href="#/pull/' + pr.id + '" class="file-link">' + pr.id + '</a></td>' +
                            '<td><a href="#/pull/' + pr.id + '">' + escapeHtml(pr.title) + '</a></td>' +
                            '<td><span class="issue-status-badge ' + sc + '">' + escapeHtml(pr.status) + '</span></td>' +
                            '<td class="mono">' + escapeHtml(pr.source_branch) + ' &rarr; ' + escapeHtml(pr.target_branch) + '</td>' +
                            '<td>' + escapeHtml(pr.author) + '</td>' +
                            '</tr>';
                    });
                    html += '</tbody></table></div>';
                    el.innerHTML = html;
                }
                show('repo-pulls-content');
            } catch (e) {
                var el = document.getElementById('repo-pulls-content');
                if (el) el.innerHTML = '<span style="color:var(--text-secondary)">Could not load pull requests</span>';
                hide('repo-pulls-loading');
                show('repo-pulls-content');
            }
        })();

        (async function () {
            try {
                var visData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/visibility');
                var visLabel = document.getElementById('repo-visibility-label');
                if (visLabel) visLabel.textContent = visData.visibility || 'public';
            } catch (e) {
                var visLabel = document.getElementById('repo-visibility-label');
                if (visLabel) visLabel.textContent = 'public';
            }
        })();

        contentEl.querySelector('#btn-toggle-visibility').addEventListener('click', async function () {
            var visLabel = document.getElementById('repo-visibility-label');
            var current = (visLabel.textContent || 'public').trim();
            var next = current === 'public' ? 'private' : 'public';
            try {
                await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/visibility', {
                    method: 'PATCH',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ visibility: next }),
                });
                visLabel.textContent = next;
                toast('Visibility set to ' + next, 'success');
            } catch (err) {
                toast('Failed to change visibility: ' + err.message, 'error');
            }
        });

        contentEl.querySelector('#form-fork').addEventListener('submit', async function (e) {
            e.preventDefault();
            var target = document.getElementById('fork-target').value.trim();
            if (!target) return;
            try {
                await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/fork', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ source_repo_id: repoId, target_repo_id: target }),
                });
                toast('Forked to ' + target, 'success');
                document.getElementById('fork-target').value = '';
            } catch (err) {
                toast('Failed to fork: ' + err.message, 'error');
            }
        });

        (async function () {
            try {
                var wikiData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/wiki');
                var pages = wikiData.pages || [];
                var el = document.getElementById('repo-wiki-content');
                hide('repo-wiki-loading');
                if (pages.length === 0) {
                    el.innerHTML = '<span style="color:var(--text-secondary)">No wiki pages</span>';
                } else {
                    var html = '<div class="table-wrap"><table><thead><tr><th>Title</th><th>Author</th><th>Updated</th></tr></thead><tbody>';
                    pages.slice(0, 5).forEach(function (p) {
                        html += '<tr>' +
                            '<td><a href="#/repo/' + encodeURIComponent(repoId) + '/wiki/' + encodeURIComponent(p.title) + '">' + escapeHtml(p.title) + '</a></td>' +
                            '<td>' + escapeHtml(p.author) + '</td>' +
                            '<td>' + formatTimestamp(p.updated_at) + '</td></tr>';
                    });
                    html += '</tbody></table></div>';
                    el.innerHTML = html;
                }
                show('repo-wiki-content');
            } catch (e) {
                var el = document.getElementById('repo-wiki-content');
                if (el) el.innerHTML = '<span style="color:var(--text-secondary)">Could not load wiki</span>';
                hide('repo-wiki-loading');
                show('repo-wiki-content');
            }
        })();

        (async function () {
            try {
                var relData = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/releases');
                var releases = relData.releases || [];
                var el = document.getElementById('repo-releases-content');
                hide('repo-releases-loading');
                if (releases.length === 0) {
                    el.innerHTML = '<span style="color:var(--text-secondary)">No releases</span>';
                } else {
                    var html = '<div class="table-wrap"><table><thead><tr><th>Tag</th><th>Title</th><th>Author</th><th>Pre-release</th><th>Created</th></tr></thead><tbody>';
                    releases.slice(0, 5).forEach(function (r) {
                        html += '<tr>' +
                            '<td class="mono">' + escapeHtml(r.tag) + '</td>' +
                            '<td>' + escapeHtml(r.title) + '</td>' +
                            '<td>' + escapeHtml(r.author) + '</td>' +
                            '<td>' + (r.prerelease ? 'Yes' : 'No') + '</td>' +
                            '<td>' + formatTimestamp(r.created_at) + '</td></tr>';
                    });
                    html += '</tbody></table></div>';
                    el.innerHTML = html;
                }
                show('repo-releases-content');
            } catch (e) {
                var el = document.getElementById('repo-releases-content');
                if (el) el.innerHTML = '<span style="color:var(--text-secondary)">Could not load releases</span>';
                hide('repo-releases-loading');
                show('repo-releases-content');
            }
        })();

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

    (async function () {
        try {
            var allRepos = await fetchJSON(API_BASE + '/repos');
            var repoList = (allRepos.repos || []);
            var codeResults = [];
            for (var i = 0; i < repoList.length && codeResults.length < 20; i++) {
                var rid = repoList[i].repo_id || repoList[i].name || '';
                try {
                    var cs = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(rid) + '/search/code?q=' + encodeURIComponent(query));
                    (cs.results || []).forEach(function (r) { r._repoId = rid; codeResults.push(r); });
                } catch (e) { }
            }
            if (codeResults.length > 0) {
                var codeHtml = '<h3>Code Results</h3>';
                codeResults.forEach(function (r) {
                    codeHtml += '<div class="search-item">' +
                        '<a href="#/repo/' + encodeURIComponent(r._repoId) + '">' + escapeHtml(r.path || r.blob_hash) + '</a>' +
                        '<div class="search-item-meta">' +
                        (r.path ? escapeHtml(r.path) + ' \u2014 ' : '') +
                        escapeHtml(r._repoId) +
                        ' (' + r.match_count + ' matches)</div>' +
                        '<pre style="margin:0.25rem 0 0;padding:0.5rem;background:var(--bg-secondary);border-radius:4px;font-size:0.85em;overflow:auto;max-height:120px">' + escapeHtml(r.snippet) + '</pre>' +
                        '</div>';
                });
                var container = document.getElementById('search-results');
                if (container) {
                    container.innerHTML += codeHtml;
                }
            }
        } catch (e) { }
    })();
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
        '</div>' +
        '<div class="settings-panel hidden" id="settings-orgs">' +
        '<h3 style="margin-bottom:0.5rem">Organizations</h3>' +
        '<div id="settings-orgs-list" style="margin-bottom:1rem"></div>' +
        '<form id="form-create-org" style="display:flex;gap:0.5rem;align-items:end;flex-wrap:wrap">' +
        '<div><label for="org-name">Name</label><input type="text" id="org-name" required autocomplete="off"></div>' +
        '<div><label for="org-display-name">Display Name</label><input type="text" id="org-display-name" autocomplete="off"></div>' +
        '<button type="submit" class="btn btn-primary">Create Org</button></form>' +
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

        (async function () {
            try {
                var orgsData = await fetchJSON(API_BASE + '/orgs');
                var orgs = orgsData.orgs || [];
                var el = document.getElementById('settings-orgs-list');
                if (orgs.length === 0) {
                    el.innerHTML = '<span style="color:var(--text-secondary)">No organizations</span>';
                } else {
                    var html = '<div class="table-wrap"><table><thead><tr><th>Name</th><th>Display Name</th><th>Description</th><th></th></tr></thead><tbody>';
                    orgs.forEach(function (o) {
                        html += '<tr>' +
                            '<td>' + escapeHtml(o.name) + '</td>' +
                            '<td>' + escapeHtml(o.display_name) + '</td>' +
                            '<td>' + escapeHtml(o.description) + '</td>' +
                            '<td><button class="btn btn-danger btn-sm btn-delete-org" data-org-id="' + o.id + '">Delete</button></td></tr>';
                    });
                    html += '</tbody></table></div>';
                    el.innerHTML = html;
                }
                show('settings-orgs');
            } catch (e) {
                document.getElementById('settings-orgs-list').innerHTML = '<span style="color:var(--text-secondary)">Could not load organizations</span>';
                show('settings-orgs');
            }
        })();

        container.querySelector('#form-create-org').addEventListener('submit', async function (e) {
            e.preventDefault();
            var name = document.getElementById('org-name').value.trim();
            var displayName = document.getElementById('org-display-name').value.trim();
            if (!name) return;
            try {
                await fetchJSON(API_BASE + '/orgs', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: name, display_name: displayName || name, description: '' }),
                });
                toast('Organization "' + name + '" created', 'success');
                document.getElementById('org-name').value = '';
                document.getElementById('org-display-name').value = '';
                router();
            } catch (err) {
                toast('Failed to create org: ' + err.message, 'error');
            }
        });

        container.addEventListener('click', async function (e) {
            var btn = e.target.closest('.btn-delete-org');
            if (!btn) return;
            var orgId = btn.dataset.orgId;
            var ok = await confirmDialog('Delete Organization', 'Delete this organization?');
            if (!ok) return;
            try {
                await fetchJSON(API_BASE + '/orgs/' + orgId, { method: 'DELETE' });
                toast('Organization deleted', 'success');
                router();
            } catch (err) {
                toast('Failed to delete org: ' + err.message, 'error');
            }
        });
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

async function renderIssues(container, repoId) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Issues</span></div>' +
        '<div class="section-header"><h2>Issues</h2>' +
        '<button class="btn btn-primary" id="btn-new-issue">+ New Issue</button></div>' +
        '<div class="form-panel hidden" id="new-issue-form">' +
        '<h3>Create Issue</h3>' +
        '<form id="form-new-issue">' +
        '<label for="issue-title">Title</label>' +
        '<input type="text" id="issue-title" placeholder="Issue title" required autocomplete="off">' +
        '<label for="issue-body">Body</label>' +
        '<textarea id="issue-body" rows="4" placeholder="Describe the issue..."></textarea>' +
        '<label for="issue-labels">Labels (comma-separated)</label>' +
        '<input type="text" id="issue-labels" placeholder="bug, enhancement" autocomplete="off">' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-new-issue">Cancel</button>' +
        '</div></form></div>' +
        '<div id="issues-loading" class="loading">Loading issues&hellip;</div>' +
        '<div id="issues-error" class="error-banner hidden"></div>' +
        '<div id="issues-content" class="hidden"></div>' +
        '<div id="issues-empty" class="empty-state hidden">No issues found.</div>';

    container.querySelector('#btn-new-issue').addEventListener('click', function () { togglePanel('new-issue-form'); });
    container.querySelector('#cancel-new-issue').addEventListener('click', function () { hide('new-issue-form'); });
    container.querySelector('#form-new-issue').addEventListener('submit', async function (e) {
        e.preventDefault();
        var title = document.getElementById('issue-title').value.trim();
        var body = document.getElementById('issue-body').value.trim();
        var labelsStr = document.getElementById('issue-labels').value.trim();
        var labels = labelsStr ? labelsStr.split(',').map(function (l) { return l.trim(); }).filter(Boolean) : [];
        if (!title) return;
        try {
            await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/issues', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ repo_id: repoId, title: title, body: body, labels: labels }),
            });
            toast('Issue created', 'success');
            document.getElementById('form-new-issue').reset();
            hide('new-issue-form');
            loadIssuesData();
        } catch (err) {
            toast('Failed to create issue: ' + err.message, 'error');
        }
    });

    await loadIssuesData();
}

async function loadIssuesData() {
    hide('issues-error');
    hide('issues-content');
    hide('issues-empty');
    show('issues-loading');

    var repoId = window.location.hash.match(/^#\/repo\/([^/]+)\/issues$/);
    if (!repoId) return;
    repoId = decodeURIComponent(repoId[1]);

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/issues');
        var issues = data.issues || [];

        hide('issues-loading');

        if (issues.length === 0) {
            show('issues-empty');
            return;
        }

        var html = '<div class="table-wrap"><table><thead><tr>' +
            '<th>#</th><th>Title</th><th>Status</th><th>Author</th><th>Labels</th><th>Updated</th>' +
            '</tr></thead><tbody>';

        issues.forEach(function (issue) {
            var statusClass = issue.status === 'open' ? 'status-open' : 'status-closed';
            var labelBadges = (issue.labels || []).map(function (l) {
                return '<span class="label-badge">' + escapeHtml(l) + '</span>';
            }).join(' ');
            html += '<tr>' +
                '<td><a href="#/issue/' + issue.id + '" class="file-link">' + issue.id + '</a></td>' +
                '<td><a href="#/issue/' + issue.id + '">' + escapeHtml(issue.title) + '</a></td>' +
                '<td><span class="issue-status-badge ' + statusClass + '">' + escapeHtml(issue.status) + '</span></td>' +
                '<td>' + escapeHtml(issue.author) + '</td>' +
                '<td>' + labelBadges + '</td>' +
                '<td>' + formatTimestamp(issue.updated_at) + '</td>' +
                '</tr>';
        });

        html += '</tbody></table></div>';

        document.getElementById('issues-content').innerHTML = html;
        show('issues-content');
    } catch (err) {
        hide('issues-loading');
        var banner = document.getElementById('issues-error');
        banner.textContent = 'Failed to load issues: ' + err.message;
        show('issues-error');
    }
}

async function renderIssueDetail(container, issueId) {
    container.innerHTML =
        '<div class="breadcrumb" id="issue-breadcrumb"></div>' +
        '<div id="issue-loading" class="loading">Loading issue&hellip;</div>' +
        '<div id="issue-error" class="error-banner hidden"></div>' +
        '<div id="issue-content" class="hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/issues/' + issueId);
        if (!data.success || !data.issue) throw new Error('Issue not found');
        var issue = data.issue;

        hide('issue-loading');

        var bc = document.getElementById('issue-breadcrumb');
        bc.innerHTML =
            '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(issue.repo_id) + '">' + escapeHtml(issue.repo_id) + '</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(issue.repo_id) + '/issues">Issues</a> <span class="separator">/</span> ' +
            '<span class="current">#' + issue.id + '</span>';

        var statusClass = issue.status === 'open' ? 'status-open' : 'status-closed';
        var labelBadges = (issue.labels || []).map(function (l) {
            return '<span class="label-badge">' + escapeHtml(l) + '</span>';
        }).join(' ');
        var toggleBtnLabel = issue.status === 'open' ? 'Close Issue' : 'Reopen Issue';
        var toggleBtnClass = issue.status === 'open' ? 'btn-danger' : 'btn-primary';
        var toggleStatus = issue.status === 'open' ? 'closed' : 'open';

        var el = document.getElementById('issue-content');
        el.innerHTML =
            '<div class="issue-header">' +
            '<h2><span class="issue-status-badge ' + statusClass + '">' + escapeHtml(issue.status) + '</span> #' + issue.id + ': ' + escapeHtml(issue.title) + '</h2>' +
            '<div class="issue-meta">' +
            '<span>Author: ' + escapeHtml(issue.author) + '</span>' +
            '<span>Created: ' + formatTimestamp(issue.created_at) + '</span>' +
            '<span>Updated: ' + formatTimestamp(issue.updated_at) + '</span>' +
            (issue.closed_at ? '<span>Closed: ' + formatTimestamp(issue.closed_at) + '</span>' : '') +
            '</div>' +
            '<div class="issue-labels">' + labelBadges + '</div>' +
            '</div>' +
            '<div class="issue-body"><pre>' + escapeHtml(issue.body || '') + '</pre></div>' +
            '<div class="issue-actions">' +
            '<button class="btn ' + toggleBtnClass + '" id="btn-toggle-issue" data-status="' + toggleStatus + '">' + escapeHtml(toggleBtnLabel) + '</button>' +
            '</div>' +
            '<div class="issue-comments-section">' +
            '<h3>Comments</h3>' +
            '<div id="comments-loading" class="loading">Loading comments&hellip;</div>' +
            '<div id="comments-list" class="hidden"></div>' +
            '<div class="comment-form">' +
            '<form id="form-comment">' +
            '<textarea id="comment-body" rows="3" placeholder="Write a comment..." required></textarea>' +
            '<div class="form-actions">' +
            '<button type="submit" class="btn btn-primary">Comment</button>' +
            '</div></form></div>' +
            '</div>';

        show('issue-content');

        el.querySelector('#btn-toggle-issue').addEventListener('click', async function () {
            var btn = el.querySelector('#btn-toggle-issue');
            var newStatus = btn.dataset.status;
            try {
                await fetchJSON(API_BASE + '/issues/' + issueId, {
                    method: 'PATCH',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ status: newStatus }),
                });
                toast('Issue ' + newStatus, 'success');
                router();
            } catch (err) {
                toast('Failed to update issue: ' + err.message, 'error');
            }
        });

        el.querySelector('#form-comment').addEventListener('submit', async function (e) {
            e.preventDefault();
            var body = document.getElementById('comment-body').value.trim();
            if (!body) return;
            try {
                await fetchJSON(API_BASE + '/issues/' + issueId + '/comments', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ body: body }),
                });
                toast('Comment added', 'success');
                document.getElementById('comment-body').value = '';
                loadComments();
            } catch (err) {
                toast('Failed to add comment: ' + err.message, 'error');
            }
        });

        async function loadComments() {
            try {
                var cData = await fetchJSON(API_BASE + '/issues/' + issueId + '/comments');
                var comments = cData.comments || [];
                hide('comments-loading');
                var cList = document.getElementById('comments-list');
                if (comments.length === 0) {
                    cList.innerHTML = '<p style="color:var(--text-secondary)">No comments yet.</p>';
                } else {
                    var cHtml = '';
                    comments.forEach(function (c) {
                        cHtml += '<div class="comment-item">' +
                            '<div class="comment-header">' +
                            '<span class="comment-author">' + escapeHtml(c.author) + '</span>' +
                            '<span class="comment-time">' + formatTimestamp(c.created_at) + '</span>' +
                            '</div>' +
                            '<div class="comment-body"><pre>' + escapeHtml(c.body) + '</pre></div>' +
                            '</div>';
                    });
                    cList.innerHTML = cHtml;
                }
                show('comments-list');
            } catch (err) {
                hide('comments-loading');
                document.getElementById('comments-list').innerHTML = '<p style="color:var(--text-secondary)">Could not load comments.</p>';
                show('comments-list');
            }
        }

        loadComments();
    } catch (err) {
        hide('issue-loading');
        var errEl = document.getElementById('issue-error');
        errEl.textContent = 'Failed to load issue: ' + err.message;
        show('issue-error');
    }
}

checkConnection();
setInterval(checkConnection, 30000);
router();

async function renderPulls(container, repoId) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Pull Requests</span></div>' +
        '<div class="section-header"><h2>Pull Requests</h2>' +
        '<button class="btn btn-primary" id="btn-new-pr">+ New Pull Request</button></div>' +
        '<div class="form-panel hidden" id="new-pr-form">' +
        '<h3>Create Pull Request</h3>' +
        '<form id="form-new-pr">' +
        '<label for="pr-title">Title</label>' +
        '<input type="text" id="pr-title" placeholder="PR title" required autocomplete="off">' +
        '<label for="pr-body">Body</label>' +
        '<textarea id="pr-body" rows="4" placeholder="Describe the changes..."></textarea>' +
        '<label for="pr-source">Source Branch</label>' +
        '<input type="text" id="pr-source" placeholder="feature-branch" required autocomplete="off">' +
        '<label for="pr-target">Target Branch</label>' +
        '<input type="text" id="pr-target" placeholder="main" autocomplete="off">' +
        '<div class="form-actions">' +
        '<button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-new-pr">Cancel</button>' +
        '</div></form></div>' +
        '<div id="pulls-loading" class="loading">Loading pull requests&hellip;</div>' +
        '<div id="pulls-error" class="error-banner hidden"></div>' +
        '<div id="pulls-content" class="hidden"></div>' +
        '<div id="pulls-empty" class="empty-state hidden">No pull requests found.</div>';

    container.querySelector('#btn-new-pr').addEventListener('click', function () { togglePanel('new-pr-form'); });
    container.querySelector('#cancel-new-pr').addEventListener('click', function () { hide('new-pr-form'); });
    container.querySelector('#form-new-pr').addEventListener('submit', async function (e) {
        e.preventDefault();
        var title = document.getElementById('pr-title').value.trim();
        var body = document.getElementById('pr-body').value.trim();
        var source = document.getElementById('pr-source').value.trim();
        var target = document.getElementById('pr-target').value.trim();
        if (!title || !source) return;
        try {
            await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/pulls', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ repo_id: repoId, title: title, body: body, source_branch: source, target_branch: target || undefined }),
            });
            toast('Pull request created', 'success');
            document.getElementById('form-new-pr').reset();
            hide('new-pr-form');
            loadPullsData();
        } catch (err) {
            toast('Failed to create pull request: ' + err.message, 'error');
        }
    });

    await loadPullsData();
}

async function loadPullsData() {
    hide('pulls-error');
    hide('pulls-content');
    hide('pulls-empty');
    show('pulls-loading');

    var repoId = window.location.hash.match(/^#\/repo\/([^/]+)\/pulls$/);
    if (!repoId) return;
    repoId = decodeURIComponent(repoId[1]);

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/pulls');
        var pulls = data.pulls || [];

        hide('pulls-loading');

        if (pulls.length === 0) {
            show('pulls-empty');
            return;
        }

        var html = '<div class="table-wrap"><table><thead><tr>' +
            '<th>#</th><th>Title</th><th>Status</th><th>Branch</th><th>Author</th><th>Updated</th>' +
            '</tr></thead><tbody>';

        pulls.forEach(function (pr) {
            var sc = pr.status === 'open' ? 'status-open' : pr.status === 'merged' ? 'pr-status-merged' : 'status-closed';
            html += '<tr>' +
                '<td><a href="#/pull/' + pr.id + '" class="file-link">' + pr.id + '</a></td>' +
                '<td><a href="#/pull/' + pr.id + '">' + escapeHtml(pr.title) + '</a></td>' +
                '<td><span class="issue-status-badge ' + sc + '">' + escapeHtml(pr.status) + '</span></td>' +
                '<td class="mono">' + escapeHtml(pr.source_branch) + ' &rarr; ' + escapeHtml(pr.target_branch) + '</td>' +
                '<td>' + escapeHtml(pr.author) + '</td>' +
                '<td>' + formatTimestamp(pr.updated_at) + '</td>' +
                '</tr>';
        });

        html += '</tbody></table></div>';

        document.getElementById('pulls-content').innerHTML = html;
        show('pulls-content');
    } catch (err) {
        hide('pulls-loading');
        var banner = document.getElementById('pulls-error');
        banner.textContent = 'Failed to load pull requests: ' + err.message;
        show('pulls-error');
    }
}

async function renderPullDetail(container, pullId) {
    container.innerHTML =
        '<div class="breadcrumb" id="pull-breadcrumb"></div>' +
        '<div id="pull-loading" class="loading">Loading pull request&hellip;</div>' +
        '<div id="pull-error" class="error-banner hidden"></div>' +
        '<div id="pull-content" class="hidden"></div>';

    try {
        var data = await fetchJSON(API_BASE + '/pulls/' + pullId);
        if (!data.success || !data.pull_request) throw new Error('Pull request not found');
        var pr = data.pull_request;

        hide('pull-loading');

        var bc = document.getElementById('pull-breadcrumb');
        bc.innerHTML =
            '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(pr.repo_id) + '">' + escapeHtml(pr.repo_id) + '</a> <span class="separator">/</span> ' +
            '<a href="#/repo/' + encodeURIComponent(pr.repo_id) + '/pulls">Pull Requests</a> <span class="separator">/</span> ' +
            '<span class="current">#' + pr.id + '</span>';

        var sc = pr.status === 'open' ? 'status-open' : pr.status === 'merged' ? 'pr-status-merged' : 'status-closed';
        var actionsHtml = '';
        if (pr.status === 'open') {
            actionsHtml =
                '<button class="btn btn-primary" id="btn-merge-pr">Merge</button> ' +
                '<button class="btn btn-danger" id="btn-close-pr">Close</button>';
        }

        var el = document.getElementById('pull-content');
        el.innerHTML =
            '<div class="issue-header">' +
            '<h2><span class="issue-status-badge ' + sc + '">' + escapeHtml(pr.status) + '</span> #' + pr.id + ': ' + escapeHtml(pr.title) + '</h2>' +
            '<div class="issue-meta">' +
            '<span>Author: ' + escapeHtml(pr.author) + '</span>' +
            '<span>Branch: ' + escapeHtml(pr.source_branch) + ' &rarr; ' + escapeHtml(pr.target_branch) + '</span>' +
            '<span>Created: ' + formatTimestamp(pr.created_at) + '</span>' +
            '<span>Updated: ' + formatTimestamp(pr.updated_at) + '</span>' +
            (pr.merged_at ? '<span>Merged: ' + formatTimestamp(pr.merged_at) + '</span>' : '') +
            (pr.merged_by ? '<span>Merged by: ' + escapeHtml(pr.merged_by) + '</span>' : '') +
            '</div></div>' +
            '<div class="issue-body"><pre>' + escapeHtml(pr.body || '') + '</pre></div>' +
            '<div class="issue-actions" id="pr-actions">' + actionsHtml + '</div>' +
            '<div class="issue-comments-section">' +
            '<h3>Reviews</h3>' +
            '<div id="reviews-loading" class="loading">Loading reviews&hellip;</div>' +
            '<div id="reviews-list" class="hidden"></div>' +
            (pr.status === 'open' ?
            '<div class="comment-form">' +
            '<form id="form-review">' +
            '<label for="review-verdict">Verdict</label>' +
            '<select id="review-verdict">' +
            '<option value="approve">Approve</option>' +
            '<option value="request_changes">Request Changes</option>' +
            '<option value="comment">Comment</option>' +
            '</select>' +
            '<textarea id="review-body" rows="3" placeholder="Write a review comment..."></textarea>' +
            '<div class="form-actions">' +
            '<button type="submit" class="btn btn-primary">Submit Review</button>' +
            '</div></form></div>' : '') +
            '</div>';

        show('pull-content');

        if (pr.status === 'open') {
            var mergeBtn = document.getElementById('btn-merge-pr');
            if (mergeBtn) {
                mergeBtn.addEventListener('click', async function () {
                    try {
                        await fetchJSON(API_BASE + '/pulls/' + pullId + '/merge', { method: 'POST', headers: { 'Content-Type': 'application/json' } });
                        toast('Pull request merged', 'success');
                        router();
                    } catch (err) {
                        toast('Failed to merge: ' + err.message, 'error');
                    }
                });
            }
            var closeBtn = document.getElementById('btn-close-pr');
            if (closeBtn) {
                closeBtn.addEventListener('click', async function () {
                    try {
                        await fetchJSON(API_BASE + '/pulls/' + pullId + '/close', { method: 'POST', headers: { 'Content-Type': 'application/json' } });
                        toast('Pull request closed', 'success');
                        router();
                    } catch (err) {
                        toast('Failed to close: ' + err.message, 'error');
                    }
                });
            }
        }

        var reviewForm = document.getElementById('form-review');
        if (reviewForm) {
            reviewForm.addEventListener('submit', async function (e) {
                e.preventDefault();
                var verdict = document.getElementById('review-verdict').value;
                var body = document.getElementById('review-body').value.trim();
                try {
                    await fetchJSON(API_BASE + '/pulls/' + pullId + '/reviews', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ verdict: verdict, body: body }),
                    });
                    toast('Review submitted', 'success');
                    document.getElementById('review-body').value = '';
                    loadReviews();
                } catch (err) {
                    toast('Failed to submit review: ' + err.message, 'error');
                }
            });
        }

        async function loadReviews() {
            try {
                var rData = await fetchJSON(API_BASE + '/pulls/' + pullId + '/reviews');
                var reviews = rData.reviews || [];
                hide('reviews-loading');
                var rList = document.getElementById('reviews-list');
                if (reviews.length === 0) {
                    rList.innerHTML = '<p style="color:var(--text-secondary)">No reviews yet.</p>';
                } else {
                    var rHtml = '';
                    reviews.forEach(function (r) {
                        var verdictClass = r.verdict === 'approve' ? 'status-open' : r.verdict === 'request_changes' ? 'status-closed' : 'pr-status-merged';
                        rHtml += '<div class="comment-item">' +
                            '<div class="comment-header">' +
                            '<span class="comment-author">' + escapeHtml(r.reviewer) + '</span>' +
                            '<span class="issue-status-badge ' + verdictClass + '" style="margin-left:0.5rem">' + escapeHtml(r.verdict) + '</span>' +
                            '<span class="comment-time">' + formatTimestamp(r.created_at) + '</span>' +
                            '</div>' +
                            '<div class="comment-body"><pre>' + escapeHtml(r.body) + '</pre></div>' +
                            '</div>';
                    });
                    rList.innerHTML = rHtml;
                }
                show('reviews-list');
            } catch (err) {
                hide('reviews-loading');
                document.getElementById('reviews-list').innerHTML = '<p style="color:var(--text-secondary)">Could not load reviews.</p>';
                show('reviews-list');
            }
        }

        loadReviews();
    } catch (err) {
        hide('pull-loading');
        var errEl = document.getElementById('pull-error');
        errEl.textContent = 'Failed to load pull request: ' + err.message;
        show('pull-error');
    }
}

async function renderWiki(container, repoId, pageTitle) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Wiki</span></div>' +
        '<div class="section-header"><h2>Wiki — ' + escapeHtml(repoId) + '</h2>' +
        '<button class="btn btn-primary btn-sm" id="btn-new-wiki-page">+ New Page</button></div>' +
        '<div id="wiki-new-form" class="form-panel hidden" style="margin-bottom:1rem">' +
        '<form id="form-create-wiki">' +
        '<label for="wiki-new-title">Title</label><input type="text" id="wiki-new-title" required autocomplete="off">' +
        '<label for="wiki-new-content">Content</label><textarea id="wiki-new-content" rows="6" style="width:100%;box-sizing:border-box"></textarea>' +
        '<div class="form-actions"><button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-new-wiki">Cancel</button></div></form></div>' +
        '<div id="wiki-page-list"></div>' +
        '<div id="wiki-page-view" class="hidden"></div>';

    container.querySelector('#btn-new-wiki-page').addEventListener('click', function () { togglePanel('wiki-new-form'); });
    container.querySelector('#cancel-new-wiki').addEventListener('click', function () { hide('wiki-new-form'); });
    container.querySelector('#form-create-wiki').addEventListener('submit', async function (e) {
        e.preventDefault();
        var title = document.getElementById('wiki-new-title').value.trim();
        var content = document.getElementById('wiki-new-content').value;
        if (!title) return;
        try {
            await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/wiki', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ title: title, content: content }),
            });
            toast('Wiki page "' + title + '" created', 'success');
            router();
        } catch (err) {
            toast('Failed to create wiki page: ' + err.message, 'error');
        }
    });

    if (pageTitle) {
        hide('wiki-page-list');
        try {
            var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/wiki/' + encodeURIComponent(pageTitle));
            var page = data.page;
            show('wiki-page-view');
            document.getElementById('wiki-page-view').innerHTML =
                '<div class="repo-header"><h3>' + escapeHtml(page.title) + '</h3>' +
                '<div><span style="color:var(--text-secondary)">by ' + escapeHtml(page.author) + ' \u2014 ' + formatTimestamp(page.updated_at) + '</span></div></div>' +
                '<div style="margin-top:1rem"><pre style="white-space:pre-wrap;word-break:break-word">' + escapeHtml(page.content) + '</pre></div>' +
                '<div style="margin-top:1rem"><form id="form-edit-wiki">' +
                '<label>Edit Content</label><textarea id="wiki-edit-content" rows="8" style="width:100%;box-sizing:border-box">' + escapeHtml(page.content) + '</textarea>' +
                '<div class="form-actions" style="margin-top:0.5rem"><button type="submit" class="btn btn-primary">Save</button></div></form></div>';
            document.getElementById('form-edit-wiki').addEventListener('submit', async function (e) {
                e.preventDefault();
                var newContent = document.getElementById('wiki-edit-content').value;
                try {
                    await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/wiki/' + encodeURIComponent(pageTitle), {
                        method: 'PUT',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ title: pageTitle, content: newContent }),
                    });
                    toast('Wiki page updated', 'success');
                    router();
                } catch (err) {
                    toast('Failed to update wiki: ' + err.message, 'error');
                }
            });
        } catch (err) {
            document.getElementById('wiki-page-view').innerHTML = '<div class="error-banner">Page not found</div>';
            show('wiki-page-view');
        }
    } else {
        try {
            var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/wiki');
            var pages = data.pages || [];
            var el = document.getElementById('wiki-page-list');
            if (pages.length === 0) {
                el.innerHTML = '<span style="color:var(--text-secondary)">No wiki pages yet. Click "+ New Page" to create one.</span>';
            } else {
                var html = '<div class="table-wrap"><table><thead><tr><th>Title</th><th>Author</th><th>Updated</th></tr></thead><tbody>';
                pages.forEach(function (p) {
                    html += '<tr>' +
                        '<td><a href="#/repo/' + encodeURIComponent(repoId) + '/wiki/' + encodeURIComponent(p.title) + '">' + escapeHtml(p.title) + '</a></td>' +
                        '<td>' + escapeHtml(p.author) + '</td>' +
                        '<td>' + formatTimestamp(p.updated_at) + '</td></tr>';
                });
                html += '</tbody></table></div>';
                el.innerHTML = html;
            }
        } catch (err) {
            document.getElementById('wiki-page-list').innerHTML = '<span style="color:var(--text-secondary)">Could not load wiki</span>';
        }
    }
}

async function renderReleases(container, repoId) {
    container.innerHTML =
        '<div class="breadcrumb">' +
        '<a href="#/repos">Repos</a> <span class="separator">/</span> ' +
        '<a href="#/repo/' + encodeURIComponent(repoId) + '">' + escapeHtml(repoId) + '</a> <span class="separator">/</span> ' +
        '<span class="current">Releases</span></div>' +
        '<div class="section-header"><h2>Releases — ' + escapeHtml(repoId) + '</h2>' +
        '<button class="btn btn-primary btn-sm" id="btn-new-release">+ New Release</button></div>' +
        '<div id="release-new-form" class="form-panel hidden" style="margin-bottom:1rem">' +
        '<form id="form-create-release">' +
        '<label for="rel-tag">Tag</label><input type="text" id="rel-tag" placeholder="v1.0.0" required autocomplete="off">' +
        '<label for="rel-title">Title</label><input type="text" id="rel-title" required autocomplete="off">' +
        '<label for="rel-body">Description</label><textarea id="rel-body" rows="4" style="width:100%;box-sizing:border-box"></textarea>' +
        '<div style="margin:0.5rem 0"><label><input type="checkbox" id="rel-prerelease"> Pre-release</label></div>' +
        '<div class="form-actions"><button type="submit" class="btn btn-primary">Create</button>' +
        '<button type="button" class="btn btn-secondary" id="cancel-new-release">Cancel</button></div></form></div>' +
        '<div id="releases-loading" class="loading">Loading releases&hellip;</div>' +
        '<div id="releases-list"></div>';

    container.querySelector('#btn-new-release').addEventListener('click', function () { togglePanel('release-new-form'); });
    container.querySelector('#cancel-new-release').addEventListener('click', function () { hide('release-new-form'); });
    container.querySelector('#form-create-release').addEventListener('submit', async function (e) {
        e.preventDefault();
        var tag = document.getElementById('rel-tag').value.trim();
        var title = document.getElementById('rel-title').value.trim();
        var body = document.getElementById('rel-body').value;
        var prerelease = document.getElementById('rel-prerelease').checked;
        if (!tag || !title) return;
        try {
            await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/releases', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ repo_id: repoId, tag: tag, title: title, body: body, prerelease: prerelease }),
            });
            toast('Release "' + tag + '" created', 'success');
            router();
        } catch (err) {
            toast('Failed to create release: ' + err.message, 'error');
        }
    });

    try {
        var data = await fetchJSON(API_BASE + '/repos/' + encodeURIComponent(repoId) + '/releases');
        hide('releases-loading');
        var releases = data.releases || [];
        var el = document.getElementById('releases-list');
        if (releases.length === 0) {
            el.innerHTML = '<span style="color:var(--text-secondary)">No releases yet. Click "+ New Release" to create one.</span>';
        } else {
            var html = '';
            releases.forEach(function (r) {
                var preBadge = r.prerelease ? '<span class="issue-status-badge status-closed" style="margin-left:0.5rem">pre-release</span>' : '';
                html += '<div class="search-item" style="flex-direction:column;align-items:start">' +
                    '<div style="display:flex;align-items:center;gap:0.5rem">' +
                    '<span class="mono" style="font-weight:bold">' + escapeHtml(r.tag) + '</span>' +
                    '<span>' + escapeHtml(r.title) + '</span>' + preBadge + '</div>' +
                    '<div class="search-item-meta">' + escapeHtml(r.author) + ' \u2014 ' + formatTimestamp(r.created_at) + '</div>' +
                    (r.body ? '<p style="margin:0.25rem 0 0;color:var(--text-secondary)">' + escapeHtml(r.body) + '</p>' : '') +
                    '</div>';
            });
            el.innerHTML = html;
        }
    } catch (err) {
        hide('releases-loading');
        document.getElementById('releases-list').innerHTML = '<span style="color:var(--text-secondary)">Could not load releases</span>';
    }
}
