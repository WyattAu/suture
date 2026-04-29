use axum::{
    response::{Html, IntoResponse, Response},
};

pub async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub async fn serve_static() -> Response {
    serve_index().await.into_response()
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Suture — Semantic Merge Platform</title>
    <style>
        :root {
            --bg: #0a0a0f;
            --surface: #12121a;
            --surface-2: #1a1a25;
            --border: #2a2a3a;
            --text: #e0e0e8;
            --text-muted: #8888a0;
            --primary: #6366f1;
            --primary-hover: #818cf8;
            --success: #22c55e;
            --warning: #f59e0b;
            --danger: #ef4444;
            --font-mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
            --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
        }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: var(--font-sans); background: var(--bg); color: var(--text); line-height: 1.6; }
        a { color: var(--primary); text-decoration: none; }
        a:hover { color: var(--primary-hover); }
        nav {
            display: flex; align-items: center; justify-content: space-between;
            padding: 0.75rem 1.5rem; border-bottom: 1px solid var(--border);
            background: var(--surface);
        }
        .nav-brand { font-weight: 700; font-size: 1.25rem; display: flex; align-items: center; gap: 0.5rem; }
        .nav-brand svg { width: 24px; height: 24px; }
        .nav-links { display: flex; gap: 1.5rem; align-items: center; }
        .nav-links a { color: var(--text-muted); font-size: 0.9rem; }
        .nav-links a:hover { color: var(--text); }
        .btn {
            display: inline-flex; align-items: center; gap: 0.5rem;
            padding: 0.5rem 1rem; border-radius: 6px; border: 1px solid var(--border);
            background: var(--surface-2); color: var(--text); cursor: pointer;
            font-size: 0.9rem; font-family: var(--font-sans); transition: all 0.15s;
        }
        .btn:hover { border-color: var(--primary); background: var(--primary); }
        .btn-primary { background: var(--primary); border-color: var(--primary); }
        .btn-primary:hover { background: var(--primary-hover); border-color: var(--primary-hover); }
        .btn-sm { padding: 0.3rem 0.75rem; font-size: 0.8rem; }
        .btn-danger { border-color: var(--danger); color: var(--danger); }
        .btn-danger:hover { background: var(--danger); color: white; }
        .container { max-width: 1200px; margin: 0 auto; padding: 2rem 1.5rem; }
        .grid { display: grid; gap: 1.5rem; }
        .grid-2 { grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); }
        .grid-3 { grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); }
        .card {
            background: var(--surface); border: 1px solid var(--border);
            border-radius: 8px; padding: 1.5rem;
        }
        .card h3 { font-size: 1rem; margin-bottom: 0.75rem; }
        .card-value { font-size: 2rem; font-weight: 700; }
        .hero { text-align: center; padding: 4rem 0; }
        .hero h1 { font-size: 3rem; font-weight: 800; margin-bottom: 1rem; }
        .hero h1 span { color: var(--primary); }
        .hero p { color: var(--text-muted); font-size: 1.1rem; max-width: 600px; margin: 0 auto 2rem; }
        .form-group { margin-bottom: 1rem; }
        .form-group label { display: block; font-size: 0.85rem; color: var(--text-muted); margin-bottom: 0.3rem; }
        .form-group input, .form-group select, .form-group textarea {
            width: 100%; padding: 0.5rem 0.75rem; border-radius: 6px;
            border: 1px solid var(--border); background: var(--surface-2);
            color: var(--text); font-family: var(--font-sans); font-size: 0.9rem;
        }
        .form-group textarea { font-family: var(--font-mono); resize: vertical; min-height: 200px; }
        .form-group input:focus, .form-group select:focus, .form-group textarea:focus {
            outline: none; border-color: var(--primary);
        }
        .auth-page { max-width: 400px; margin: 4rem auto; }
        .auth-page h2 { text-align: center; margin-bottom: 1.5rem; }
        .merge-editor { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; }
        .merge-pane { border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
        .merge-pane-header {
            padding: 0.5rem 0.75rem; font-size: 0.8rem; font-weight: 600;
            border-bottom: 1px solid var(--border); background: var(--surface);
        }
        .merge-pane-content { padding: 0.75rem; font-family: var(--font-mono); font-size: 0.85rem; white-space: pre-wrap; }
        .merge-result {
            grid-column: 1 / -1; border: 2px solid var(--primary);
            border-radius: 6px; overflow: hidden;
        }
        .merge-result-header {
            padding: 0.5rem 0.75rem; font-size: 0.8rem; font-weight: 600;
            background: var(--primary); color: white;
            display: flex; justify-content: space-between; align-items: center;
        }
        .merge-result-content { padding: 0.75rem; font-family: var(--font-mono); font-size: 0.85rem; white-space: pre-wrap; min-height: 100px; }
        .badge {
            display: inline-flex; padding: 0.15rem 0.5rem; border-radius: 9999px;
            font-size: 0.75rem; font-weight: 600;
        }
        .badge-success { background: rgba(34,197,94,0.15); color: var(--success); }
        .badge-warning { background: rgba(245,158,11,0.15); color: var(--warning); }
        .badge-danger { background: rgba(239,68,68,0.15); color: var(--danger); }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid var(--border); }
        th { font-size: 0.8rem; color: var(--text-muted); font-weight: 600; }
        .usage-bar { height: 6px; background: var(--surface-2); border-radius: 3px; overflow: hidden; margin-top: 0.5rem; }
        .usage-bar-fill { height: 100%; border-radius: 3px; transition: width 0.3s; }
        .usage-bar-fill.green { background: var(--success); }
        .usage-bar-fill.yellow { background: var(--warning); }
        .usage-bar-fill.red { background: var(--danger); }
        @media (max-width: 768px) {
            .merge-editor { grid-template-columns: 1fr; }
            .hero h1 { font-size: 2rem; }
        }
        .api-endpoint {
            border: 1px solid var(--border); border-radius: 6px;
            margin-bottom: 1rem; overflow: hidden;
        }
        .api-endpoint-header {
            display: flex; align-items: center; gap: 0.75rem;
            padding: 0.75rem; background: var(--surface-2);
            font-family: var(--font-mono); font-size: 0.85rem;
        }
        .api-method {
            padding: 0.15rem 0.5rem; border-radius: 4px;
            font-weight: 700; font-size: 0.75rem;
        }
        .api-method-post { background: rgba(34,197,94,0.2); color: var(--success); }
        .api-method-get { background: rgba(99,102,241,0.2); color: var(--primary); }
        .api-endpoint-body { padding: 0.75rem; font-size: 0.85rem; }
    </style>
</head>
<body>
    <nav>
        <div class="nav-brand">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
            </svg>
            Suture
        </div>
        <div class="nav-links">
            <a href="#merge">Merge</a>
            <a href="#api">API</a>
            <a href="#pricing">Pricing</a>
            <a href="#" onclick="showAuth('login')" class="btn btn-sm">Sign In</a>
            <a href="#" onclick="showAuth('register')" class="btn btn-sm btn-primary">Sign Up</a>
        </div>
    </nav>

    <div class="hero">
        <h1>Semantic Merge<br>for <span>Every Format</span></h1>
        <p>Automatically resolve merge conflicts in JSON, YAML, TOML, XML, CSV, and 12+ more formats. No more manual conflict resolution.</p>
        <div style="display:flex;gap:1rem;justify-content:center">
            <a href="#merge" class="btn btn-primary">Try Merge</a>
            <a href="#api" class="btn">View API</a>
        </div>
    </div>

    <div class="container" id="merge">
        <h2 style="margin-bottom:1rem">3-Way Semantic Merge</h2>
        <div class="form-group" style="max-width:300px">
            <label>Format</label>
            <select id="merge-driver">
                <option value="json">JSON</option>
                <option value="yaml">YAML</option>
                <option value="toml">TOML</option>
                <option value="xml">XML</option>
                <option value="csv">CSV</option>
            </select>
        </div>
        <div class="merge-editor">
            <div class="merge-pane">
                <div class="merge-pane-header">Base</div>
                <div class="merge-pane-content">
                    <textarea id="merge-base" style="width:100%;min-height:250px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste base version...">{
  "name": "suture",
  "version": "5.0.1",
  "features": ["merge", "diff"]
}</textarea>
                </div>
            </div>
            <div class="merge-pane">
                <div class="merge-pane-header" style="color:var(--primary)">Ours</div>
                <div class="merge-pane-content">
                    <textarea id="merge-ours" style="width:100%;min-height:250px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste our version...">{
  "name": "suture",
  "version": "5.1.0",
  "features": ["merge", "diff", "platform"]
}</textarea>
                </div>
            </div>
            <div class="merge-pane">
                <div class="merge-pane-header" style="color:var(--success)">Theirs</div>
                <div class="merge-pane-content">
                    <textarea id="merge-theirs" style="width:100%;min-height:250px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste their version...">{
  "name": "suture",
  "version": "5.0.1",
  "features": ["merge", "diff"],
  "license": "AGPL-3.0"
}</textarea>
                </div>
            </div>
            <div class="merge-result">
                <div class="merge-result-header">
                    <span>Merged Result</span>
                    <div style="display:flex;gap:0.5rem">
                        <span id="merge-status" class="badge badge-success">Ready</span>
                        <button class="btn btn-sm" onclick="doMerge()">Merge</button>
                        <button class="btn btn-sm" onclick="copyResult()">Copy</button>
                    </div>
                </div>
                <div class="merge-result-content" id="merge-result">Click "Merge" to resolve conflicts automatically...</div>
            </div>
        </div>
    </div>

    <div class="container" id="api" style="margin-top:3rem">
        <h2 style="margin-bottom:1rem">REST API</h2>
        <p style="color:var(--text-muted);margin-bottom:1.5rem">All endpoints accept <code>Authorization: Bearer &lt;token&gt;</code>. Use the merge API programmatically.</p>
        <div class="api-endpoint">
            <div class="api-endpoint-header">
                <span class="api-method api-method-post">POST</span>
                <span>/auth/register</span>
            </div>
            <div class="api-endpoint-body">
                Register a new account. Body: <code>{"email": "...", "password": "..."}</code>
            </div>
        </div>
        <div class="api-endpoint">
            <div class="api-endpoint-header">
                <span class="api-method api-method-post">POST</span>
                <span>/auth/login</span>
            </div>
            <div class="api-endpoint-body">
                Login and get JWT token. Body: <code>{"email": "...", "password": "..."}</code>
            </div>
        </div>
        <div class="api-endpoint">
            <div class="api-endpoint-header">
                <span class="api-method api-method-post">POST</span>
                <span>/api/merge</span>
            </div>
            <div class="api-endpoint-body">
                3-way semantic merge. Body: <code>{"driver": "json", "base": "...", "ours": "...", "theirs": "..."}</code>
            </div>
        </div>
        <div class="api-endpoint">
            <div class="api-endpoint-header">
                <span class="api-method api-method-get">GET</span>
                <span>/api/drivers</span>
            </div>
            <div class="api-endpoint-body">
                List supported merge drivers and their file extensions.
            </div>
        </div>
        <div class="api-endpoint">
            <div class="api-endpoint-header">
                <span class="api-method api-method-get">GET</span>
                <span>/api/usage</span>
            </div>
            <div class="api-endpoint-body">
                Get current billing period usage and limits.
            </div>
        </div>
    </div>

    <div class="container" id="pricing" style="margin-top:3rem">
        <h2 style="text-align:center;margin-bottom:2rem">Pricing</h2>
        <div class="grid grid-3">
            <div class="card">
                <h3>Free</h3>
                <div class="card-value">$0</div>
                <p style="color:var(--text-muted);margin:0.5rem 0">For individuals and small projects</p>
                <ul style="list-style:none;margin:1rem 0;font-size:0.9rem">
                    <li>5 repositories</li>
                    <li>100 merges/month</li>
                    <li>100 MB storage</li>
                    <li>5 core drivers</li>
                </ul>
            </div>
            <div class="card" style="border-color:var(--primary)">
                <h3>Pro <span class="badge badge-success">Popular</span></h3>
                <div class="card-value">$9<span style="font-size:0.9rem;font-weight:400;color:var(--text-muted)">/user/mo</span></div>
                <p style="color:var(--text-muted);margin:0.5rem 0">For teams and growing projects</p>
                <ul style="list-style:none;margin:1rem 0;font-size:0.9rem">
                    <li>Unlimited repositories</li>
                    <li>10,000 merges/month</li>
                    <li>10 GB storage</li>
                    <li>All 17+ drivers</li>
                    <li>7-day audit log</li>
                </ul>
            </div>
            <div class="card">
                <h3>Enterprise</h3>
                <div class="card-value">$29<span style="font-size:0.9rem;font-weight:400;color:var(--text-muted)">/user/mo</span></div>
                <p style="color:var(--text-muted);margin:0.5rem 0">For organizations with compliance needs</p>
                <ul style="list-style:none;margin:1rem 0;font-size:0.9rem">
                    <li>Unlimited everything</li>
                    <li>100 GB storage</li>
                    <li>SAML/SSO</li>
                    <li>Unlimited audit log</li>
                    <li>99.99% SLA</li>
                    <li>Priority support</li>
                </ul>
            </div>
        </div>
        <p style="text-align:center;color:var(--text-muted);margin-top:1.5rem;font-size:0.9rem">
            Self-hosted Suture is always free. <a href="https://github.com/WyattAu/suture">View on GitHub</a>
        </p>
    </div>

    <footer style="border-top:1px solid var(--border);padding:2rem 1.5rem;margin-top:3rem;text-align:center">
        <p style="color:var(--text-muted);font-size:0.85rem">
            Suture - Semantic Merge Platform -
            <a href="https://github.com/WyattAu/suture">GitHub</a> -
            <a href="#">Docs</a> -
            <a href="#">Status</a>
        </p>
    </footer>

    <script>
    function showAuth(mode) {
        var email = prompt('Email:');
        if (!email) return;
        var password = prompt('Password (min 8 chars):');
        if (!password) return;
        var endpoint = mode === 'login' ? '/auth/login' : '/auth/register';
        fetch(endpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email: email, password: password })
        })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            if (data.token) {
                localStorage.setItem('suture_token', data.token);
                alert('Welcome! Token saved.');
                updateNav(data.user);
            } else {
                alert('Error: ' + (data.error || JSON.stringify(data)));
            }
        })
        .catch(function(e) { alert('Network error: ' + e); });
    }

    function updateNav(user) {
        var links = document.querySelector('.nav-links');
        if (user) {
            links.innerHTML =
                '<a href="#merge">Merge</a>' +
                '<a href="#api">API</a>' +
                '<a href="#pricing">Pricing</a>' +
                '<span style="color:var(--text-muted);font-size:0.85rem">' + user.email + ' (' + user.tier + ')</span>' +
                '<a href="#" onclick="logout()" class="btn btn-sm">Sign Out</a>';
        }
    }

    function logout() {
        localStorage.removeItem('suture_token');
        location.reload();
    }

    async function doMerge() {
        var driver = document.getElementById('merge-driver').value;
        var base = document.getElementById('merge-base').value;
        var ours = document.getElementById('merge-ours').value;
        var theirs = document.getElementById('merge-theirs').value;
        var resultEl = document.getElementById('merge-result');
        var statusEl = document.getElementById('merge-status');
        statusEl.textContent = 'Merging...';
        statusEl.className = 'badge badge-warning';
        try {
            var token = localStorage.getItem('suture_token');
            var headers = { 'Content-Type': 'application/json' };
            if (token) headers['Authorization'] = 'Bearer ' + token;
            var resp = await fetch('/api/merge', {
                method: 'POST',
                headers: headers,
                body: JSON.stringify({ driver: driver, base: base, ours: ours, theirs: theirs })
            });
            var data = await resp.json();
            if (data.result) {
                resultEl.textContent = data.result;
                statusEl.textContent = 'Merged';
                statusEl.className = 'badge badge-success';
            } else {
                resultEl.textContent = 'Conflicts detected - automatic merge not possible. Manual resolution required.';
                statusEl.textContent = 'Conflicts';
                statusEl.className = 'badge badge-danger';
            }
            if (resp.status === 401) {
                resultEl.textContent += '\n\nSign in to use the merge API (anonymous merges are rate-limited).';
            }
        } catch (e) {
            resultEl.textContent = 'Error: ' + e.message;
            statusEl.textContent = 'Error';
            statusEl.className = 'badge badge-danger';
        }
    }

    function copyResult() {
        var text = document.getElementById('merge-result').textContent;
        navigator.clipboard.writeText(text).then(function() {
            var btn = event.target;
            btn.textContent = 'Copied!';
            setTimeout(function() { btn.textContent = 'Copy'; }, 1500);
        });
    }

    var savedToken = localStorage.getItem('suture_token');
    if (savedToken) {
        fetch('/auth/me', { headers: { 'Authorization': 'Bearer ' + savedToken } })
        .then(function(r) { return r.ok ? r.json() : null; })
        .then(function(user) { if (user) updateNav(user.user || user); })
        .catch(function() {});
    }
    </script>
</body>
</html>"##;
