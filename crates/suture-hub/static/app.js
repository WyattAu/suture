const API_BASE = '';

async function fetchJSON(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    return response.json();
}

async function loadRepos() {
    try {
        const data = await fetchJSON(`${API_BASE}/repos`);
        const repos = data.repo_ids || [];
        const select = document.getElementById('repo-select');
        repos.forEach(repo => {
            const option = document.createElement('option');
            option.value = repo;
            option.textContent = repo;
            select.appendChild(option);
        });
    } catch (err) {
        console.error('Failed to load repos:', err);
    }
}

async function selectRepo(repoName) {
    if (!repoName) {
        document.getElementById('repo-info').classList.add('hidden');
        document.getElementById('empty-state').classList.remove('hidden');
        return;
    }

    document.getElementById('repo-info').classList.remove('hidden');
    document.getElementById('empty-state').classList.add('hidden');

    try {
        const branches = await fetchJSON(`${API_BASE}/repos/${encodeURIComponent(repoName)}/branches`);
        const branchList = document.getElementById('branch-list');
        branchList.innerHTML = '';

        branches.forEach(branch => {
            const li = document.createElement('li');
            li.textContent = branch.name;
            li.dataset.tip = branch.target_id.value || branch.target_id;
            li.onclick = () => selectBranch(repoName, branch.name, li);
            branchList.appendChild(li);
        });

        if (branches.length > 0) {
            const firstLi = branchList.querySelector('li');
            selectBranch(repoName, branches[0].name, firstLi);
        }
    } catch (err) {
        console.error('Failed to load repo:', err);
    }
}

async function selectBranch(repoName, branchName, element) {
    document.querySelectorAll('#branch-list li').forEach(li => li.classList.remove('active'));
    if (element) element.classList.add('active');

    try {
        const patches = await fetchJSON(`${API_BASE}/repos/${encodeURIComponent(repoName)}/patches`);

        const logDiv = document.getElementById('commit-log');
        logDiv.innerHTML = '';

        patches.forEach(patch => {
            const id = patch.id.value || patch.id;
            const entry = document.createElement('div');
            entry.className = 'commit-entry';
            entry.innerHTML = `
                <div class="hash">${id.substring(0, 12)}</div>
                <div class="message">${escapeHtml(patch.message)}</div>
                <div class="meta">${escapeHtml(patch.author)} &middot; ${new Date(patch.timestamp * 1000).toLocaleString()}</div>
            `;
            entry.onclick = () => showCommit(patch);
            logDiv.appendChild(entry);
        });
    } catch (err) {
        console.error('Failed to load patches:', err);
    }
}

async function showCommit(patch) {
    const id = patch.id.value || patch.id;
    document.getElementById('detail-title').textContent = `Commit ${id.substring(0, 12)}`;

    const parentStr = (patch.parent_ids || [])
        .map(p => (p.value || p).substring(0, 12))
        .join(', ') || 'none (root)';

    const touchStr = Array.isArray(patch.touch_set)
        ? patch.touch_set.join(', ')
        : (patch.touch_set || 'none');

    const detail = document.getElementById('commit-detail');
    detail.innerHTML = `
        <div class="field">
            <div class="field-label">Full Hash</div>
            <div class="field-value">${id}</div>
        </div>
        <div class="field">
            <div class="field-label">Author</div>
            <div class="field-value">${escapeHtml(patch.author)}</div>
        </div>
        <div class="field">
            <div class="field-label">Timestamp</div>
            <div class="field-value">${new Date(patch.timestamp * 1000).toISOString()}</div>
        </div>
        <div class="field">
            <div class="field-label">Operation</div>
            <div class="field-value">${escapeHtml(patch.operation_type)}</div>
        </div>
        <div class="field">
            <div class="field-label">Touch Set</div>
            <div class="field-value">${escapeHtml(touchStr)}</div>
        </div>
        <div class="field">
            <div class="field-label">Target Path</div>
            <div class="field-value">${escapeHtml(patch.target_path || 'none')}</div>
        </div>
        <div class="field">
            <div class="field-label">Parents</div>
            <div class="field-value">${parentStr}</div>
        </div>
        <div class="field">
            <div class="field-label">Message</div>
            <div class="field-value">${escapeHtml(patch.message)}</div>
        </div>
    `;
}

function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

document.getElementById('repo-select').addEventListener('change', (e) => {
    selectRepo(e.target.value);
});

loadRepos();
