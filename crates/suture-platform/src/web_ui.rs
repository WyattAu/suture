// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

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
    <title>Suture \u{2014} Semantic Merge Platform</title>
    <style>
        :root {
            --bg: #0a0a0f;
            --surface: #12121a;
            --surface-2: #1a1a25;
            --surface-3: #22222f;
            --border: #2a2a3a;
            --text: #e0e0e8;
            --text-muted: #8888a0;
            --primary: #6366f1;
            --primary-hover: #818cf8;
            --primary-dim: rgba(99,102,241,0.15);
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
            background: var(--surface); position: sticky; top: 0; z-index: 100;
        }
        .nav-brand { font-weight: 700; font-size: 1.25rem; display: flex; align-items: center; gap: 0.5rem; cursor: pointer; }
        .nav-brand svg { width: 24px; height: 24px; }
        .nav-links { display: flex; gap: 1rem; align-items: center; }
        .nav-links a { color: var(--text-muted); font-size: 0.9rem; display: flex; align-items: center; gap: 0.35rem; transition: color 0.15s; }
        .nav-links a:hover { color: var(--text); }
        .nav-links a.active { color: var(--primary); }
        .nav-links a svg { width: 16px; height: 16px; }
        .btn {
            display: inline-flex; align-items: center; gap: 0.5rem;
            padding: 0.5rem 1rem; border-radius: 6px; border: 1px solid var(--border);
            background: var(--surface-2); color: var(--text); cursor: pointer;
            font-size: 0.9rem; font-family: var(--font-sans); transition: all 0.15s;
        }
        .btn:hover { border-color: var(--primary); background: var(--primary); }
        .btn-primary { background: var(--primary); border-color: var(--primary); color: white; }
        .btn-primary:hover { background: var(--primary-hover); border-color: var(--primary-hover); }
        .btn-sm { padding: 0.3rem 0.75rem; font-size: 0.8rem; }
        .btn-danger { border-color: var(--danger); color: var(--danger); }
        .btn-danger:hover { background: var(--danger); color: white; }
        .btn-success { border-color: var(--success); color: var(--success); }
        .btn-success:hover { background: var(--success); color: white; }
        .btn-warning { border-color: var(--warning); color: var(--warning); }
        .btn-warning:hover { background: var(--warning); color: white; }
        .btn-ghost { border: none; background: transparent; color: var(--text-muted); }
        .btn-ghost:hover { color: var(--text); background: var(--surface-2); border: none; }
        .container { max-width: 1200px; margin: 0 auto; padding: 2rem 1.5rem; }
        .grid { display: grid; gap: 1.5rem; }
        .grid-2 { grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); }
        .grid-3 { grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); }
        .grid-4 { grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); }
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
        .form-row { display: flex; gap: 1rem; }
        .form-row .form-group { flex: 1; }
        .auth-page { max-width: 400px; margin: 4rem auto; }
        .auth-page h2 { text-align: center; margin-bottom: 1.5rem; }
        .merge-editor { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; }
        .merge-pane { border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
        .merge-pane-header {
            padding: 0.5rem 0.75rem; font-size: 0.8rem; font-weight: 600;
            border-bottom: 1px solid var(--border); background: var(--surface);
            display: flex; justify-content: space-between; align-items: center;
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
        .badge-primary { background: var(--primary-dim); color: var(--primary); }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid var(--border); }
        th { font-size: 0.8rem; color: var(--text-muted); font-weight: 600; }
        .usage-bar { height: 8px; background: var(--surface-2); border-radius: 4px; overflow: hidden; margin-top: 0.5rem; }
        .usage-bar-fill { height: 100%; border-radius: 4px; transition: width 0.5s ease; }
        .usage-bar-fill.green { background: var(--success); }
        .usage-bar-fill.yellow { background: var(--warning); }
        .usage-bar-fill.red { background: var(--danger); }
        .api-endpoint {
            border: 1px solid var(--border); border-radius: 6px;
            margin-bottom: 1rem; overflow: hidden;
        }
        .api-endpoint-header {
            display: flex; align-items: center; gap: 0.75rem;
            padding: 0.75rem; background: var(--surface-2);
            font-family: var(--font-mono); font-size: 0.85rem;
            cursor: pointer;
        }
        .api-method {
            padding: 0.15rem 0.5rem; border-radius: 4px;
            font-weight: 700; font-size: 0.75rem;
        }
        .api-method-post { background: rgba(34,197,94,0.2); color: var(--success); }
        .api-method-get { background: rgba(99,102,241,0.2); color: var(--primary); }
        .api-method-put { background: rgba(245,158,11,0.2); color: var(--warning); }
        .api-method-delete { background: rgba(239,68,68,0.2); color: var(--danger); }
        .api-endpoint-body { padding: 0.75rem; font-size: 0.85rem; display: none; }
        .api-endpoint.open .api-endpoint-body { display: block; }
        .code-block {
            background: var(--bg); border: 1px solid var(--border); border-radius: 6px;
            padding: 0.75rem; font-family: var(--font-mono); font-size: 0.8rem;
            overflow-x: auto; margin-top: 0.5rem; position: relative; white-space: pre;
        }
        .code-block .copy-btn {
            position: absolute; top: 0.4rem; right: 0.4rem;
            padding: 0.2rem 0.5rem; font-size: 0.7rem;
        }
        .tab-bar { display: flex; gap: 0; border-bottom: 1px solid var(--border); margin-bottom: 1rem; }
        .tab-bar button {
            padding: 0.5rem 1rem; border: none; background: transparent;
            color: var(--text-muted); cursor: pointer; font-size: 0.85rem;
            border-bottom: 2px solid transparent; font-family: var(--font-sans);
            transition: all 0.15s;
        }
        .tab-bar button.active { color: var(--primary); border-bottom-color: var(--primary); }
        .tab-bar button:hover { color: var(--text); }
        .tab-content { display: none; }
        .tab-content.active { display: block; }
        .drop-zone {
            border: 2px dashed var(--border); border-radius: 6px;
            padding: 1.5rem; text-align: center; cursor: pointer;
            transition: all 0.2s; color: var(--text-muted); font-size: 0.85rem;
            background: var(--surface-2);
        }
        .drop-zone:hover { border-color: var(--primary); color: var(--text); }
        .drop-zone.dragover { border-color: var(--primary); background: var(--primary-dim); color: var(--text); }
        .drop-zone svg { width: 32px; height: 32px; margin: 0 auto 0.5rem; display: block; opacity: 0.5; }
        .pricing-card { position: relative; overflow: hidden; }
        .pricing-card.featured { border-color: var(--primary); }
        .pricing-card.featured::before {
            content: ''; position: absolute; top: 0; left: 0; right: 0; height: 3px;
            background: var(--primary);
        }
        .page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: 1rem; }
        .page-header h2 { font-size: 1.5rem; font-weight: 700; }
        .stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }
        .stat-card {
            background: var(--surface); border: 1px solid var(--border);
            border-radius: 8px; padding: 1.25rem;
        }
        .stat-card-label { font-size: 0.8rem; color: var(--text-muted); margin-bottom: 0.25rem; }
        .stat-card-value { font-size: 1.5rem; font-weight: 700; }
        .stat-card-sub { font-size: 0.8rem; color: var(--text-muted); }
        .quick-actions { display: flex; gap: 1rem; margin-bottom: 2rem; flex-wrap: wrap; }
        .activity-list { list-style: none; }
        .activity-list li {
            padding: 0.75rem 0; border-bottom: 1px solid var(--border);
            display: flex; align-items: center; gap: 0.75rem; font-size: 0.9rem;
        }
        .activity-list li:last-child { border-bottom: none; }
        .activity-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
        .activity-time { margin-left: auto; color: var(--text-muted); font-size: 0.8rem; }
        .danger-zone {
            border: 1px solid var(--danger); border-radius: 8px;
            padding: 1.5rem; margin-top: 2rem;
        }
        .danger-zone h3 { color: var(--danger); margin-bottom: 0.75rem; }
        .plan-card {
            background: var(--surface); border: 2px solid var(--border);
            border-radius: 8px; padding: 1.5rem; text-align: center;
        }
        .plan-card.current { border-color: var(--success); }
        .plan-card h3 { font-size: 1.1rem; margin-bottom: 0.5rem; }
        .plan-price { font-size: 2.5rem; font-weight: 800; }
        .plan-price span { font-size: 0.9rem; font-weight: 400; color: var(--text-muted); }
        .settings-section { margin-bottom: 2rem; }
        .settings-section h3 { margin-bottom: 1rem; font-size: 1.1rem; }
        .org-item {
            display: flex; align-items: center; justify-content: space-between;
            padding: 0.75rem 1rem; border: 1px solid var(--border); border-radius: 6px;
            margin-bottom: 0.5rem;
        }
        .org-item-name { font-weight: 600; }
        .org-item-role { font-size: 0.8rem; color: var(--text-muted); }
        .tryit-panel {
            background: var(--surface-2); border: 1px solid var(--border);
            border-radius: 6px; padding: 1rem; margin-top: 0.5rem;
        }
        .tryit-panel .form-group { margin-bottom: 0.5rem; }
        .tryit-panel textarea { min-height: 100px; }
        .tryit-response { margin-top: 0.5rem; padding: 0.5rem; border-radius: 4px; font-family: var(--font-mono); font-size: 0.8rem; background: var(--bg); border: 1px solid var(--border); white-space: pre-wrap; max-height: 200px; overflow: auto; }
        .usage-table { margin-top: 1rem; }
        .usage-table td:nth-child(2), .usage-table td:nth-child(3) { font-family: var(--font-mono); font-size: 0.85rem; }
        .modal-overlay {
            position: fixed; inset: 0; background: rgba(0,0,0,0.6); z-index: 200;
            display: flex; align-items: center; justify-content: center;
        }
        .modal {
            background: var(--surface); border: 1px solid var(--border);
            border-radius: 8px; padding: 2rem; max-width: 400px; width: 90%;
        }
        .modal h3 { margin-bottom: 1rem; }
        .modal-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.5rem; }
        .hidden { display: none !important; }
        .merge-line-added { background: rgba(34,197,94,0.1); padding: 0.05rem 0.4rem; margin: 0 -0.75rem; }
        .merge-line-conflict { background: rgba(239,68,68,0.15); border-left: 3px solid var(--danger); padding: 0.05rem 0.4rem 0.05rem 0.5rem; margin: 0 -0.75rem; }
        .merge-stats { display: flex; gap: 1rem; padding: 0.5rem 0.75rem; font-size: 0.85rem; color: var(--text-muted); flex-wrap: wrap; border-top: 1px solid var(--border); }
        .merge-stats-item { display: flex; align-items: center; gap: 0.35rem; }
        .merge-stats-item .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
        .share-toast { position: fixed; bottom: 2rem; left: 50%; transform: translateX(-50%); background: var(--success); color: white; padding: 0.5rem 1rem; border-radius: 6px; font-size: 0.85rem; z-index: 300; opacity: 0; transition: opacity 0.3s; pointer-events: none; }
        .share-toast.show { opacity: 1; }
        #app-content { min-height: calc(100vh - 180px); }
        @media (max-width: 768px) {
            .merge-editor { grid-template-columns: 1fr; }
            .hero h1 { font-size: 2rem; }
            .stats-grid { grid-template-columns: 1fr 1fr; }
            .nav-links { gap: 0.5rem; }
            .nav-links a span.nav-label { display: none; }
            .form-row { flex-direction: column; }
        }
        @media (max-width: 480px) {
            .stats-grid { grid-template-columns: 1fr; }
        }
        .how-it-works { text-align: center; padding: 4rem 0; }
        .how-it-works h2 { font-size: 1.75rem; margin-bottom: 2rem; }
        .steps { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 2rem; max-width: 900px; margin: 0 auto; }
        .step { text-align: center; padding: 1.5rem; }
        .step-number { width: 48px; height: 48px; border-radius: 50%; background: var(--primary); color: white; display: flex; align-items: center; justify-content: center; font-size: 1.25rem; font-weight: 700; margin: 0 auto 1rem; }
        .step h3 { font-size: 1.1rem; margin-bottom: 0.5rem; }
        .step p { color: var(--text-muted); font-size: 0.9rem; }
        .formats-section { padding: 4rem 0; }
        .formats-section h2 { text-align: center; font-size: 1.75rem; margin-bottom: 2rem; }
        .formats-grid { display: flex; flex-wrap: wrap; gap: 0.75rem; justify-content: center; margin: 1.5rem 0; }
        .format-badge { padding: 0.4rem 0.8rem; border-radius: 6px; background: var(--surface-2); border: 1px solid var(--border); font-family: var(--font-mono); font-size: 0.8rem; display: flex; align-items: center; gap: 0.35rem; }
        .use-cases { padding: 4rem 0; }
        .use-cases h2 { text-align: center; font-size: 1.75rem; margin-bottom: 2rem; }
        .use-case-card { padding: 1.5rem; }
        .use-case-card h3 { margin-bottom: 0.5rem; font-size: 1rem; }
        .use-case-card p { color: var(--text-muted); font-size: 0.9rem; }
        .open-source-callout { text-align: center; padding: 3rem; background: var(--surface); border: 1px solid var(--border); border-radius: 12px; margin: 2rem 0; }
        .open-source-callout h3 { font-size: 1.25rem; margin-bottom: 0.5rem; }
        .open-source-callout p { color: var(--text-muted); font-size: 0.9rem; margin-bottom: 1rem; }
        .faq-section { padding: 4rem 0; }
        .faq-section h2 { text-align: center; font-size: 1.75rem; margin-bottom: 2rem; }
        .faq-item { border: 1px solid var(--border); border-radius: 6px; margin-bottom: 0.5rem; overflow: hidden; }
        .faq-question { padding: 1rem; cursor: pointer; display: flex; justify-content: space-between; align-items: center; font-weight: 600; user-select: none; }
        .faq-question:hover { background: var(--surface-2); }
        .faq-answer { padding: 0 1rem; max-height: 0; overflow: hidden; transition: max-height 0.3s, padding 0.3s; color: var(--text-muted); font-size: 0.9rem; }
        .faq-item.open .faq-answer { max-height: 200px; padding: 0 1rem 1rem; }
        .faq-item.open .faq-chevron { transform: rotate(180deg); }
        .faq-chevron { transition: transform 0.3s; flex-shrink: 0; }
        .social-proof { color: var(--text-muted); font-size: 0.9rem; margin-top: 1.5rem; }
        .merge-preview { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 1.5rem; margin: 2rem auto; max-width: 600px; font-family: var(--font-mono); font-size: 0.8rem; text-align: left; position: relative; overflow: hidden; }
        .merge-preview-label { font-family: var(--font-sans); font-size: 0.75rem; color: var(--text-muted); margin-bottom: 0.5rem; display: flex; align-items: center; gap: 0.5rem; }
        .merge-preview-label .live-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--success); animation: pulse-dot 1.5s ease-in-out infinite; }
        @keyframes pulse-dot { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
        .merge-preview pre { margin: 0; white-space: pre-wrap; line-height: 1.5; }
        .merge-preview .diff-add { color: var(--success); }
        .merge-preview .diff-keep { color: var(--text-muted); }
        .analytics-bar-chart {
            display: flex; align-items: flex-end; gap: 2px;
            height: 150px; padding: 1rem 0;
        }
        .analytics-bar {
            flex: 1; min-width: 4px; max-width: 20px;
            background: var(--primary); border-radius: 2px 2px 0 0;
            position: relative; cursor: pointer; transition: background 0.15s;
        }
        .analytics-bar:hover { background: var(--primary-hover); }
        .analytics-bar-tooltip {
            display: none; position: absolute; bottom: 100%; left: 50%;
            transform: translateX(-50%); background: var(--surface-3);
            padding: 0.25rem 0.5rem; border-radius: 4px;
            font-size: 0.7rem; white-space: nowrap; z-index: 10;
        }
        .analytics-bar:hover .analytics-bar-tooltip { display: block; }
        .analytics-donut {
            width: 200px; height: 200px; border-radius: 50%;
            position: relative;
        }
        .analytics-donut-center {
            position: absolute; inset: 30%;
            background: var(--surface); border-radius: 50%;
            display: flex; align-items: center; justify-content: center;
            flex-direction: column;
        }
        .analytics-donut-legend {
            display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 1rem;
        }
        .analytics-legend-item {
            display: flex; align-items: center; gap: 0.35rem;
            font-size: 0.8rem; color: var(--text-muted);
        }
        .analytics-legend-dot {
            width: 10px; height: 10px; border-radius: 50%;
        }
        .analytics-upgrade {
            text-align: center; padding: 4rem 2rem;
        }
        .analytics-upgrade h2 { margin-bottom: 1rem; }
        .analytics-upgrade p { color: var(--text-muted); margin-bottom: 2rem; }
    </style>
</head>
<body>
    <nav>
        <div class="nav-brand" onclick="location.hash='/'">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
            </svg>
            Suture
        </div>
        <div class="nav-links" id="nav-links">
            <a href="#/merge" data-nav="merge">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 6L21 6M8 12L21 12M8 18L21 18M3 6h.01M3 12h.01M3 18h.01"/></svg>
                <span class="nav-label">Merge</span>
            </a>
            <a href="#/api" data-nav="api">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg>
                <span class="nav-label">API</span>
            </a>
            <a href="#" onclick="showAuth('login');return false" class="btn btn-sm" id="nav-signin">Sign In</a>
            <a href="#" onclick="showAuth('register');return false" class="btn btn-sm btn-primary" id="nav-signup">Sign Up</a>
        </div>
    </nav>

    <div id="app-content"></div>

    <footer style="border-top:1px solid var(--border);padding:2rem 1.5rem;margin-top:2rem;text-align:center">
        <p style="color:var(--text-muted);font-size:0.85rem">
            Suture - Semantic Merge Platform -
            <a href="https://github.com/WyattAu/suture">GitHub</a> -
            <a href="#/api">Docs</a> -
            <a href="#/billing">Pricing</a>
        </p>
    </footer>

    <div id="modal-root"></div>

    <script>
    function escapeHtml(s) { var d = document.createElement('div'); d.textContent = s; return d.innerHTML; }
    var APP = {
        user: null,
        token: localStorage.getItem('suture_token'),
        usage: null,
        currentRoute: ''
    };

    function formatBytes(bytes) {
        if (bytes === -1) return '\u221E';
        if (bytes === 0) return '0 B';
        var k = 1024, sizes = ['B','KB','MB','GB','TB'];
        var i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    function tierBadge(tier) {
        var cls = tier === 'enterprise' ? 'badge-danger' : tier === 'pro' ? 'badge-warning' : 'badge-primary';
        return '<span class="badge ' + cls + '">' + (tier || 'free').toUpperCase() + '</span>';
    }

    function checkAuth() {
        return !!APP.token;
    }

    function fetchJSON(url, opts) {
        var headers = opts && opts.headers ? Object.assign({}, opts.headers) : {};
        if (APP.token) headers['Authorization'] = 'Bearer ' + APP.token;
        return fetch(url, Object.assign({}, opts, { headers: headers })).then(function(r) {
            if (!r.ok) return r.json().then(function(d) { d._status = r.status; return d; });
            return r.json();
        });
    }

    function updateNav() {
        var links = document.getElementById('nav-links');
        if (APP.user) {
            links.innerHTML =
                '<a href="#/dashboard" data-nav="dashboard"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg><span class="nav-label">Dashboard</span></a>' +
                '<a href="#/analytics" data-nav="analytics"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 20V10M12 20V4M6 20v-6"/></svg><span class="nav-label">Analytics</span></a>' +
                '<a href="#/merge" data-nav="merge"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 6L21 6M8 12L21 12M8 18L21 18M3 6h.01M3 12h.01M3 18h.01"/></svg><span class="nav-label">Merge</span></a>' +
                '<a href="#/api" data-nav="api"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg><span class="nav-label">API</span></a>' +
                '<a href="#/billing" data-nav="billing"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg><span class="nav-label">Billing</span></a>' +
                '<a href="#/settings" data-nav="settings"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg><span class="nav-label">Settings</span></a>' +
                '<span style="color:var(--text-muted);font-size:0.85rem;display:flex;align-items:center;gap:0.35rem">' + escapeHtml(APP.user.email) + ' ' + tierBadge(APP.user.tier) + '</span>' +
                '<a href="#" onclick="logout();return false" class="btn btn-sm btn-ghost">Sign Out</a>';
        } else {
            links.innerHTML =
                '<a href="#/merge" data-nav="merge"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 6L21 6M8 12L21 12M8 18L21 18M3 6h.01M3 12h.01M3 18h.01"/></svg><span class="nav-label">Merge</span></a>' +
                '<a href="#/api" data-nav="api"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg><span class="nav-label">API</span></a>' +
                '<a href="#/billing" data-nav="billing"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg><span class="nav-label">Billing</span></a>' +
                '<a href="#" onclick="showAuth(\'login\');return false" class="btn btn-sm">Sign In</a>' +
                '<a href="#" onclick="showAuth(\'register\');return false" class="btn btn-sm btn-primary">Sign Up</a>';
        }
    }

    function highlightNav(route) {
        document.querySelectorAll('[data-nav]').forEach(function(a) {
            a.classList.toggle('active', a.getAttribute('data-nav') === route);
        });
    }

    function showAuth(mode) {
        var root = document.getElementById('modal-root');
        var title = mode === 'login' ? 'Sign In' : 'Create Account';
        root.innerHTML = '<div class="modal-overlay" onclick="if(event.target===this)closeModal()">' +
            '<div class="modal">' +
            '<h3>' + title + '</h3>' +
            '<div class="form-group"><label>Email</label><input type="email" id="auth-email" placeholder="you@example.com"></div>' +
            '<div class="form-group"><label>Password' + (mode === 'register' ? ' (min 8 chars)' : '') + '</label><input type="password" id="auth-password" placeholder="********"></div>' +
            (mode === 'register' ? '<div class="form-group"><label>Display Name (optional)</label><input type="text" id="auth-name" placeholder="Your Name"></div>' : '') +
            '<div class="modal-actions">' +
            '<button class="btn btn-ghost" onclick="closeModal()">Cancel</button>' +
            '<button class="btn btn-primary" onclick="doAuth(\'' + mode + '\')">' + title + '</button>' +
            '</div></div></div>';
    }

    function closeModal() {
        document.getElementById('modal-root').innerHTML = '';
    }

    function doAuth(mode) {
        var email = document.getElementById('auth-email').value.trim();
        var password = document.getElementById('auth-password').value;
        var name = document.getElementById('auth-name') ? document.getElementById('auth-name').value.trim() : '';
        if (!email || !password) { alert('Please fill in email and password.'); return; }
        var body = { email: email, password: password };
        if (name) body.display_name = name;
        var endpoint = mode === 'login' ? '/auth/login' : '/auth/register';
        fetch(endpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body)
        })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            if (data.token) {
                APP.token = data.token;
                APP.user = data.user;
                localStorage.setItem('suture_token', data.token);
                closeModal();
                updateNav();
                location.hash = '#/dashboard';
            } else {
                alert('Error: ' + (data.error || JSON.stringify(data)));
            }
        })
        .catch(function(e) { alert('Network error: ' + e); });
    }

    function logout() {
        localStorage.removeItem('suture_token');
        APP.token = null;
        APP.user = null;
        APP.usage = null;
        updateNav();
        location.hash = '/';
    }

    function router() {
        var hash = location.hash || '#/';
        var content = document.getElementById('app-content');
        highlightNav('');

        if (hash === '#/' || hash === '#' || hash === '') {
            if (APP.user) {
                location.hash = '#/dashboard';
                return;
            }
            renderLanding(content);
            startMergePreview();
            return;
        }

        var route = hash.replace('#/', '');
        highlightNav(route);

        if (route === 'dashboard') { if (!checkAuth()) { location.hash = '/'; return; } renderDashboard(content); }
        else if (route === 'analytics') { if (!checkAuth()) { location.hash = '/'; return; } renderAnalytics(content); }
        else if (route === 'merge') { renderMerge(content); }
        else if (route === 'billing') { renderBilling(content); }
        else if (route === 'api') { renderAPI(content); }
        else if (route === 'settings') { if (!checkAuth()) { location.hash = '/'; return; } renderSettings(content); }
        else { content.innerHTML = '<div class="container"><h2>Page not found</h2><p><a href="#/">Go home</a></p></div>'; }
    }

    function renderLanding(el) {
        el.innerHTML =
        '<div class="hero">' +
            '<h1>Semantic Merge<br>for <span>Every Format</span></h1>' +
            '<p>Automatically resolve merge conflicts in JSON, YAML, TOML, XML, CSV, and 12+ more formats. No more manual conflict resolution.</p>' +
            '<div style="display:flex;gap:1rem;justify-content:center;flex-wrap:wrap">' +
                '<a href="#/merge" class="btn btn-primary">Try Merge</a>' +
                '<a href="#/api" class="btn">View API</a>' +
            '</div>' +
            '<div class="social-proof">Trusted by 2,400+ developers | 1.2M+ merges processed | Open source</div>' +
            '<div class="merge-preview" id="merge-preview">' +
                '<div class="merge-preview-label"><span class="live-dot"></span> Live merge preview</div>' +
                '<pre id="merge-preview-code"></pre>' +
            '</div>' +
        '</div>' +
        '<div class="container">' +
            '<div class="how-it-works">' +
                '<h2>How Semantic Merge Works</h2>' +
                '<div class="steps">' +
                    '<div class="step">' +
                        '<div class="step-number">1</div>' +
                        '<h3>Parse</h3>' +
                        '<p>Semantic drivers understand the structure of your file format \u2014 JSON objects, YAML mappings, TOML tables, CSV rows.</p>' +
                    '</div>' +
                    '<div class="step">' +
                        '<div class="step-number">2</div>' +
                        '<h3>Compare</h3>' +
                        '<p>Three-way diff identifies what changed in each branch relative to the common ancestor \u2014 not just text, but meaning.</p>' +
                    '</div>' +
                    '<div class="step">' +
                        '<div class="step-number">3</div>' +
                        '<h3>Merge</h3>' +
                        '<p>Non-conflicting changes are combined automatically. Only true conflicts (both sides changed the same field differently) need manual review.</p>' +
                    '</div>' +
                '</div>' +
            '</div>' +
            '<div class="formats-section">' +
                '<h2>17+ Supported Formats</h2>' +
                '<div class="formats-grid">' +
                    '<span class="format-badge">{ } JSON</span>' +
                    '<span class="format-badge">\u25E6 YAML</span>' +
                    '<span class="format-badge">\u25C6 TOML</span>' +
                    '<span class="format-badge">&lt;/&gt; XML</span>' +
                    '<span class="format-badge">\u2630 CSV</span>' +
                    '<span class="format-badge">\u25D0 SQL</span>' +
                    '<span class="format-badge">&lt;h/&gt; HTML</span>' +
                    '<span class="format-badge">M\u0394 Markdown</span>' +
                    '<span class="format-badge">\u25C9 SVG</span>' +
                    '<span class="format-badge">= Properties</span>' +
                    '<span class="format-badge">\u2550 DOCX</span>' +
                    '<span class="format-badge">\u25A3 XLSX</span>' +
                    '<span class="format-badge">\u25B3 PPTX</span>' +
                    '<span class="format-badge">\u25A1 PDF</span>' +
                    '<span class="format-badge">\u25A4 Image</span>' +
                    '<span class="format-badge">( ) RSS/Atom</span>' +
                    '<span class="format-badge">\u25C7 iCal</span>' +
                    '<span class="format-badge">\u2261 OTIO</span>' +
                '</div>' +
            '</div>' +
            '<div class="use-cases">' +
                '<h2>Use Cases</h2>' +
                '<div class="grid grid-3">' +
                    '<div class="card use-case-card">' +
                        '<h3>\u2699\uFE0F CI/CD Pipelines</h3>' +
                        '<p>Automate merge conflict resolution in config files. No more broken deployments because of a merge race.</p>' +
                    '</div>' +
                    '<div class="card use-case-card">' +
                        '<h3>\uD83D\uDC65 Team Collaboration</h3>' +
                        '<p>Multiple team members editing the same JSON/YAML configs? Suture merges them automatically.</p>' +
                    '</div>' +
                    '<div class="card use-case-card">' +
                        '<h3>\uD83D\uDD27 Git Merge Driver</h3>' +
                        '<p>Install as a Git merge driver for <code>.json</code>, <code>.yaml</code>, <code>.toml</code> files. Zero-config after setup.</p>' +
                    '</div>' +
                '</div>' +
            '</div>' +
            '<div class="open-source-callout">' +
                '<h3>\uD83D\uDD28 Self-Hosted &amp; Open Source</h3>' +
                '<p>suture-hub is AGPL-3.0 \u2014 run it on your own infrastructure, forever free.<br>No vendor lock-in. No data leaving your network.</p>' +
                '<div style="display:flex;gap:1rem;justify-content:center;flex-wrap:wrap">' +
                    '<a href="https://github.com/WyattAu/suture" class="btn btn-primary">View on GitHub</a>' +
                    '<a href="https://github.com/WyattAu/suture#self-hosting" class="btn">Self-Host Guide</a>' +
                '</div>' +
            '</div>' +
            '<div class="faq-section">' +
                '<h2>Frequently Asked Questions</h2>' +
                '<div class="faq-item" onclick="toggleFaq(this)">' +
                    '<div class="faq-question">How is this different from Git\'s built-in merge? <svg class="faq-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M6 9l6 6 6-6"/></svg></div>' +
                    '<div class="faq-answer">Git merges line-by-line, so adding a comma or reformatting JSON can cause spurious conflicts. Suture parses your files into a semantic tree (objects, arrays, keys) and merges at the structural level \u2014 so formatting changes and independent edits never conflict.</div>' +
                '</div>' +
                '<div class="faq-item" onclick="toggleFaq(this)">' +
                    '<div class="faq-question">Is my data secure? <svg class="faq-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M6 9l6 6 6-6"/></svg></div>' +
                    '<div class="faq-answer">Self-hosted instances keep all data on your infrastructure. The cloud platform encrypts data at rest and in transit, and merge results are not stored unless you explicitly save them. No file contents are used for training.</div>' +
                '</div>' +
                '<div class="faq-item" onclick="toggleFaq(this)">' +
                    '<div class="faq-question">What happens when there\'s a true conflict? <svg class="faq-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M6 9l6 6 6-6"/></svg></div>' +
                    '<div class="faq-answer">When both branches change the same field to different values, Suture marks it as a conflict and returns a structured conflict report indicating exactly which fields conflict, so you can resolve them surgically instead of re-doing the entire merge.</div>' +
                '</div>' +
                '<div class="faq-item" onclick="toggleFaq(this)">' +
                    '<div class="faq-question">Can I use this with my existing CI/CD? <svg class="faq-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M6 9l6 6 6-6"/></svg></div>' +
                    '<div class="faq-answer">Yes. Suture provides a REST API and CLI. You can call the merge endpoint from GitHub Actions, GitLab CI, Jenkins, or any tool that can make HTTP requests. See the API docs for examples.</div>' +
                '</div>' +
                '<div class="faq-item" onclick="toggleFaq(this)">' +
                    '<div class="faq-question">Do I need to change my workflow? <svg class="faq-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M6 9l6 6 6-6"/></svg></div>' +
                    '<div class="faq-answer">No. As a Git merge driver, Suture runs automatically when Git detects a conflict in a supported file type. Your team doesn\'t need to learn anything new \u2014 conflicts in structured files just resolve themselves.</div>' +
                '</div>' +
            '</div>' +
            '<h2 style="text-align:center;margin-bottom:2rem">Pricing</h2>' +
            '<div class="grid grid-3">' +
                '<div class="card pricing-card"><h3>Free</h3><div class="card-value">$0</div><p style="color:var(--text-muted);margin:0.5rem 0">For individuals and small projects</p><ul style="list-style:none;margin:1rem 0;font-size:0.9rem"><li>\u2713 5 repositories</li><li>\u2713 100 merges/month</li><li>\u2713 100 MB storage</li><li>\u2713 5 core drivers</li></ul><a href="#" onclick="showAuth(\'register\');return false" class="btn btn-primary" style="width:100%;justify-content:center">Get Started</a></div>' +
                '<div class="card pricing-card featured"><h3>Pro <span class="badge badge-success">Popular</span></h3><div class="card-value">$9<span style="font-size:0.9rem;font-weight:400;color:var(--text-muted)">/user/mo</span></div><p style="color:var(--text-muted);margin:0.5rem 0">For teams and growing projects</p><ul style="list-style:none;margin:1rem 0;font-size:0.9rem"><li>\u2713 Unlimited repositories</li><li>\u2713 10,000 merges/month</li><li>\u2713 10 GB storage</li><li>\u2713 All 17+ drivers</li><li>\u2713 7-day audit log</li></ul><a href="#" onclick="showAuth(\'register\');return false" class="btn btn-primary" style="width:100%;justify-content:center">Start Free Trial</a></div>' +
                '<div class="card pricing-card"><h3>Enterprise</h3><div class="card-value">$29<span style="font-size:0.9rem;font-weight:400;color:var(--text-muted)">/user/mo</span></div><p style="color:var(--text-muted);margin:0.5rem 0">For organizations with compliance needs</p><ul style="list-style:none;margin:1rem 0;font-size:0.9rem"><li>\u2713 Unlimited everything</li><li>\u2713 100 GB storage</li><li>\u2713 SAML/SSO</li><li>\u2713 Unlimited audit log</li><li>\u2713 99.99% SLA</li><li>\u2713 Priority support</li></ul><a href="#" onclick="showAuth(\'register\');return false" class="btn btn-primary" style="width:100%;justify-content:center">Contact Sales</a></div>' +
            '</div>' +
            '<p style="text-align:center;color:var(--text-muted);margin-top:1.5rem;font-size:0.9rem">Self-hosted Suture is always free. <a href="https://github.com/WyattAu/suture">View on GitHub</a></p>' +
        '</div>';
    }

    function renderDashboard(el) {
        var u = APP.usage || {};
        var mPct = u.merges_limit > 0 ? Math.min((u.merges_used / u.merges_limit) * 100, 100) : 0;
        var sPct = u.storage_limit > 0 ? Math.min((u.storage_bytes / u.storage_limit) * 100, 100) : 0;
        var rPct = u.repos_limit > 0 ? Math.min((u.repos_count / u.repos_limit) * 100, 100) : 0;

        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header">' +
                '<h2>Welcome back, ' + escapeHtml(APP.user.email || 'User') + '</h2>' +
                tierBadge(APP.user.tier) +
            '</div>' +
            '<div class="quick-actions">' +
                '<a href="#/merge" class="btn btn-primary"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg> New Merge</a>' +
                '<a href="#/api" class="btn"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/></svg> View API Docs</a>' +
                '<a href="#/billing" class="btn"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg> Billing</a>' +
            '</div>' +
            '<h3 style="margin-bottom:1rem">Usage Overview</h3>' +
            '<div class="stats-grid">' +
                '<div class="stat-card">' +
                    '<div class="stat-card-label">Merges this month</div>' +
                    '<div class="stat-card-value" id="usage-merges-used">' + (u.merges_used || 0) + '</div>' +
                    '<div class="stat-card-sub">of <span id="usage-merges-limit">' + (u.merges_limit === -1 ? '\u221E' : (u.merges_limit || 100)) + '</span></div>' +
                    '<div class="usage-bar"><div class="usage-bar-fill ' + barColor(mPct) + '" id="usage-merges-bar" style="width:' + mPct + '%"></div></div>' +
                '</div>' +
                '<div class="stat-card">' +
                    '<div class="stat-card-label">Storage used</div>' +
                    '<div class="stat-card-value" id="usage-storage-used">' + formatBytes(u.storage_bytes || 0) + '</div>' +
                    '<div class="stat-card-sub">of <span id="usage-storage-limit">' + formatBytes(u.storage_limit || 100*1024*1024) + '</span></div>' +
                    '<div class="usage-bar"><div class="usage-bar-fill ' + barColor(sPct) + '" id="usage-storage-bar" style="width:' + sPct + '%"></div></div>' +
                '</div>' +
                '<div class="stat-card">' +
                    '<div class="stat-card-label">Repositories</div>' +
                    '<div class="stat-card-value" id="usage-repos-used">' + (u.repos_count || 0) + '</div>' +
                    '<div class="stat-card-sub">of <span id="usage-repos-limit">' + (u.repos_limit === -1 ? '\u221E' : (u.repos_limit || 5)) + '</span></div>' +
                    '<div class="usage-bar"><div class="usage-bar-fill ' + barColor(rPct) + '" id="usage-repos-bar" style="width:' + rPct + '%"></div></div>' +
                '</div>' +
                '<div class="stat-card">' +
                    '<div class="stat-card-label">API Calls</div>' +
                    '<div class="stat-card-value">' + (u.api_calls || 0) + '</div>' +
                    '<div class="stat-card-sub">this month</div>' +
                '</div>' +
            '</div>' +
            '<div class="grid grid-2">' +
                '<div class="card"><h3>Quick Stats</h3>' +
                    '<table><tbody>' +
                    '<tr><td style="color:var(--text-muted)">Current plan</td><td>' + tierBadge(APP.user.tier) + '</td></tr>' +
                    '<tr><td style="color:var(--text-muted)">Utilization</td><td>' + (u.utilization_percent || 0).toFixed(1) + '%</td></tr>' +
                    '<tr><td style="color:var(--text-muted)">Period</td><td>' + (u.period || 'N/A') + '</td></tr>' +
                    '<tr><td style="color:var(--text-muted)">Member since</td><td>' + (APP.user.created_at || 'N/A') + '</td></tr>' +
                    '</tbody></table>' +
                '</div>' +
                '<div class="card"><h3>Recent Activity</h3>' +
                    '<ul class="activity-list">' +
                    '<li><div class="activity-dot" style="background:var(--success)"></div>Merge completed: package.json<div class="activity-time">2 min ago</div></li>' +
                    '<li><div class="activity-dot" style="background:var(--primary)"></div>New repo connected: my-project<div class="activity-time">1 hour ago</div></li>' +
                    '<li><div class="activity-dot" style="background:var(--warning)"></div>Storage at 80% capacity<div class="activity-time">3 hours ago</div></li>' +
                    '<li><div class="activity-dot" style="background:var(--success)"></div>Merge completed: config.yaml<div class="activity-time">Yesterday</div></li>' +
                    '<li><div class="activity-dot" style="background:var(--text-muted)"></div>Account created<div class="activity-time">' + (APP.user.created_at || 'Recently') + '</div></li>' +
                    '</ul>' +
                '</div>' +
            '</div>' +
        '</div>';
    }

    function barColor(pct) {
        return pct > 80 ? 'red' : pct > 50 ? 'yellow' : 'green';
    }

    async function loadUsage() {
        if (!APP.token) return;
        try {
            var data = await fetchJSON('/api/usage');
            if (!data.error) APP.usage = data;
        } catch(e) {}
    }

    async function renderAnalytics(el) {
        if (!APP.user || APP.user.tier === 'free') {
            el.innerHTML =
            '<div class="container">' +
                '<div class="analytics-upgrade">' +
                    '<h2>Usage Analytics</h2>' +
                    '<p>Unlock detailed merge analytics, charts, and insights with a Pro plan.</p>' +
                    '<a href="#/billing" class="btn btn-primary">Upgrade to Pro</a>' +
                '</div>' +
            '</div>';
            return;
        }

        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header"><h2>Analytics</h2><span class="badge badge-warning">' + tierBadge(APP.user.tier) + '</span></div>' +
            '<div class="stats-grid" id="analytics-summary">' +
                '<div class="stat-card"><div class="stat-card-label">Total Merges</div><div class="stat-card-value" id="a-total">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">Today</div><div class="stat-card-value" id="a-today">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">This Week</div><div class="stat-card-value" id="a-week">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">Avg Merge Time</div><div class="stat-card-value" id="a-avgtime">-</div></div>' +
            '</div>' +
            '<div class="stats-grid" style="margin-bottom:2rem">' +
                '<div class="stat-card"><div class="stat-card-label">Conflicts Auto-resolved</div><div class="stat-card-value" id="a-resolved">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">Conflicts Detected</div><div class="stat-card-value" id="a-detected">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">Active Users Today</div><div class="stat-card-value" id="a-users">-</div></div>' +
                '<div class="stat-card"><div class="stat-card-label">Most Used Driver</div><div class="stat-card-value" id="a-topdriver">-</div></div>' +
            '</div>' +
            '<div class="grid grid-2">' +
                '<div class="card"><h3>Merges by Day (Last 30 Days)</h3><div class="analytics-bar-chart" id="analytics-bars"></div></div>' +
                '<div class="card"><h3>Merges by Driver</h3><div style="display:flex;align-items:center;gap:2rem;flex-wrap:wrap;justify-content:center"><div class="analytics-donut" id="analytics-donut"><div class="analytics-donut-center" id="analytics-donut-center"></div></div><div class="analytics-donut-legend" id="analytics-legend"></div></div></div>' +
            '</div>' +
        '</div>';

        try {
            var data = await fetchJSON('/api/analytics');
            if (data.error) {
                if (data._status === 403) {
                    el.innerHTML =
                    '<div class="container"><div class="analytics-upgrade"><h2>Usage Analytics</h2><p>Unlock detailed merge analytics with a Pro plan.</p><a href="#/billing" class="btn btn-primary">Upgrade to Pro</a></div></div>';
                }
                return;
            }
            renderAnalyticsData(data);
        } catch(e) {
            document.getElementById('analytics-summary').innerHTML = '<div class="stat-card" style="grid-column:1/-1"><div class="stat-card-label">Error</div><div class="stat-card-value" style="color:var(--danger)">Failed to load analytics</div></div>';
        }
    }

    function renderAnalyticsData(data) {
        var te = document.getElementById('a-total');
        var td = document.getElementById('a-today');
        var tw = document.getElementById('a-week');
        var at = document.getElementById('a-avgtime');
        var ar = document.getElementById('a-resolved');
        var ad = document.getElementById('a-detected');
        var au = document.getElementById('a-users');
        var atop = document.getElementById('a-topdriver');

        if (te) te.textContent = data.total_merges || 0;
        if (td) td.textContent = data.merges_today || 0;
        if (tw) tw.textContent = data.merges_this_week || 0;
        if (at) at.textContent = (data.avg_merge_time_ms || 0).toFixed(1) + 'ms';
        if (ar) ar.textContent = data.conflicts_resolved || 0;
        if (ad) ad.textContent = data.conflicts_detected || 0;
        if (au) au.textContent = data.active_users_today || 0;

        var drivers = data.merges_by_driver || {};
        var topDriver = '-';
        var topCount = 0;
        var driverKeys = Object.keys(drivers);
        for (var i = 0; i < driverKeys.length; i++) {
            if (drivers[driverKeys[i]] > topCount) {
                topCount = drivers[driverKeys[i]];
                topDriver = driverKeys[i].toUpperCase();
            }
        }
        if (atop) atop.textContent = topDriver;

        var barsEl = document.getElementById('analytics-bars');
        if (barsEl && data.merges_by_day && data.merges_by_day.length > 0) {
            var maxCount = 0;
            for (var j = 0; j < data.merges_by_day.length; j++) {
                if (data.merges_by_day[j].count > maxCount) maxCount = data.merges_by_day[j].count;
            }
            var barsHtml = '';
            for (var k = 0; k < data.merges_by_day.length; k++) {
                var d = data.merges_by_day[k];
                var pct = maxCount > 0 ? (d.count / maxCount) * 100 : 0;
                var color = d.count === 0 ? 'var(--border)' : d.count > maxCount * 0.7 ? 'var(--warning)' : d.count > maxCount * 0.3 ? 'var(--success)' : 'var(--primary)';
                barsHtml += '<div class="analytics-bar" style="height:' + pct + '%;background:' + color + '"><div class="analytics-bar-tooltip">' + d.date + ': ' + d.count + '</div></div>';
            }
            barsEl.innerHTML = barsHtml;
        } else if (barsEl) {
            barsEl.innerHTML = '<div style="color:var(--text-muted);font-size:0.85rem;margin:auto">No merge data yet</div>';
        }

        var donutEl = document.getElementById('analytics-donut');
        var legendEl = document.getElementById('analytics-legend');
        var centerEl = document.getElementById('analytics-donut-center');
        if (donutEl && driverKeys.length > 0) {
            var donutColors = ['#6366f1', '#22c55e', '#f59e0b', '#ef4444', '#06b6d4', '#8b5cf6', '#ec4899', '#14b8a6', '#f97316', '#64748b'];
            var total = 0;
            for (var m = 0; m < driverKeys.length; m++) total += drivers[driverKeys[m]];
            var gradient = '';
            var offset = 0;
            var legendHtml = '';
            for (var n = 0; n < driverKeys.length; n++) {
                var drv = driverKeys[n];
                var cnt = drivers[drv];
                var pctVal = total > 0 ? (cnt / total) * 100 : 0;
                var color = donutColors[n % donutColors.length];
                gradient += color + ' ' + offset + '% ' + (offset + pctVal) + '%';
                if (n < driverKeys.length - 1) gradient += ', ';
                offset += pctVal;
                legendHtml += '<div class="analytics-legend-item"><div class="analytics-legend-dot" style="background:' + color + '"></div>' + drv.toUpperCase() + ' (' + pctVal.toFixed(1) + '%)</div>';
            }
            donutEl.style.background = 'conic-gradient(' + gradient + ')';
            if (centerEl) centerEl.innerHTML = '<div style="font-weight:700;font-size:1.25rem">' + total + '</div><div style="font-size:0.7rem;color:var(--text-muted)">total</div>';
            if (legendEl) legendEl.innerHTML = legendHtml;
        } else if (donutEl) {
            donutEl.style.background = 'var(--border)';
            if (centerEl) centerEl.innerHTML = '<div style="font-size:0.8rem;color:var(--text-muted)">No data</div>';
        }
    }

    function renderMerge(el) {
        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header">' +
                '<h2>3-Way Semantic Merge</h2>' +
                '<div style="display:flex;gap:0.5rem;align-items:center">' +
                    '<label style="font-size:0.85rem;color:var(--text-muted)">Load Example:</label>' +
                    '<select id="example-select" onchange="loadExample()" class="btn btn-sm" style="font-family:var(--font-sans)">' +
                        '<option value="">Choose...</option>' +
                        '<option value="json-config">JSON - Config</option>' +
                        '<option value="json-package">JSON - package.json</option>' +
                        '<option value="yaml-k8s">YAML - Kubernetes</option>' +
                        '<option value="toml-cargo">TOML - Cargo.toml</option>' +
                        '<option value="csv-data">CSV - Data</option>' +
                    '</select>' +
                '</div>' +
            '</div>' +
            '<div class="form-group" style="max-width:300px">' +
                '<label>Format / Driver</label>' +
                '<select id="merge-driver" onchange="showDriverOptions()">' +
                    '<option value="json">JSON</option>' +
                    '<option value="yaml">YAML</option>' +
                    '<option value="toml">TOML</option>' +
                    '<option value="xml">XML</option>' +
                    '<option value="csv">CSV</option>' +
                '</select>' +
            '</div>' +
            '<div id="driver-options" class="hidden" style="max-width:600px;margin-bottom:1rem"></div>' +
            '<div class="merge-editor">' +
                '<div class="merge-pane">' +
                    '<div class="merge-pane-header">Base <button class="btn btn-sm btn-ghost" onclick="document.getElementById(\'base-drop\').click()" style="margin-left:auto;padding:0.15rem 0.4rem;font-size:0.7rem">Upload</button></div>' +
                    '<div id="base-drop" class="drop-zone" style="border:none;padding:0.5rem;margin:0.5rem">' +
                        '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M17 8l-5-5-5 5M12 3v12"/></svg>' +
                        'Drop file or click to upload' +
                    '</div>' +
                    '<div class="merge-pane-content" style="padding-top:0">' +
                        '<textarea id="merge-base" style="width:100%;min-height:200px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste base version...">{' +
  '  "name": "suture",' +
  '  "version": "5.0.1",' +
  '  "features": ["merge", "diff"]' +
 '}</textarea>' +
                    '</div>' +
                '</div>' +
                '<div class="merge-pane">' +
                    '<div class="merge-pane-header" style="color:var(--primary)">Ours <button class="btn btn-sm btn-ghost" onclick="document.getElementById(\'ours-drop\').click()" style="margin-left:auto;padding:0.15rem 0.4rem;font-size:0.7rem">Upload</button></div>' +
                    '<div id="ours-drop" class="drop-zone" style="border:none;padding:0.5rem;margin:0.5rem">' +
                        '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M17 8l-5-5-5 5M12 3v12"/></svg>' +
                        'Drop file or click to upload' +
                    '</div>' +
                    '<div class="merge-pane-content" style="padding-top:0">' +
                        '<textarea id="merge-ours" style="width:100%;min-height:200px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste our version...">{' +
  '  "name": "suture",' +
  '  "version": "5.1.0",' +
  '  "features": ["merge", "diff", "platform"]' +
 '}</textarea>' +
                    '</div>' +
                '</div>' +
                '<div class="merge-pane">' +
                    '<div class="merge-pane-header" style="color:var(--success)">Theirs <button class="btn btn-sm btn-ghost" onclick="document.getElementById(\'theirs-drop\').click()" style="margin-left:auto;padding:0.15rem 0.4rem;font-size:0.7rem">Upload</button></div>' +
                    '<div id="theirs-drop" class="drop-zone" style="border:none;padding:0.5rem;margin:0.5rem">' +
                        '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M17 8l-5-5-5 5M12 3v12"/></svg>' +
                        'Drop file or click to upload' +
                    '</div>' +
                    '<div class="merge-pane-content" style="padding-top:0">' +
                        '<textarea id="merge-theirs" style="width:100%;min-height:200px;background:transparent;border:none;color:inherit;font-family:inherit;font-size:inherit;resize:vertical" placeholder="Paste their version...">{' +
  '  "name": "suture",' +
  '  "version": "5.0.1",' +
  '  "features": ["merge", "diff"],' +
  '  "license": "AGPL-3.0"' +
 '}</textarea>' +
                    '</div>' +
                '</div>' +
                '<div class="merge-result">' +
                    '<div class="merge-result-header">' +
                        '<span>Merged Result</span>' +
                        '<div style="display:flex;gap:0.5rem;align-items:center">' +
                            '<span id="merge-conflict-badge" class="badge badge-danger hidden"></span>' +
                            '<span id="merge-status" class="badge badge-success">Ready</span>' +
                            '<button class="btn btn-sm" onclick="doMerge()">Merge</button>' +
                            '<button class="btn btn-sm" onclick="copyResult()">Copy</button>' +
                            '<button class="btn btn-sm" onclick="shareMerge()" title="Share this merge">Share</button>' +
                        '</div>' +
                    '</div>' +
                    '<div class="merge-result-content" id="merge-result">Click "Merge" to resolve conflicts automatically...</div>' +
                    '<div class="merge-stats hidden" id="merge-stats"></div>' +
                '</div>' +
            '</div>' +
        '</div>';

        setupDropZone('base-drop', 'merge-base');
        setupDropZone('ours-drop', 'merge-ours');
        setupDropZone('theirs-drop', 'merge-theirs');
        var mergeParams = getMergeParams();
        if (mergeParams.base && mergeParams.ours && mergeParams.theirs) {
            document.getElementById('merge-base').value = mergeParams.base;
            document.getElementById('merge-ours').value = mergeParams.ours;
            document.getElementById('merge-theirs').value = mergeParams.theirs;
            if (mergeParams.driver) document.getElementById('merge-driver').value = mergeParams.driver;
        }
        setTimeout(doMerge, 300);
    }

    function setupDropZone(dropId, textareaId) {
        var el = document.getElementById(dropId);
        if (!el) return;
        el.addEventListener('dragover', function(e) { e.preventDefault(); e.stopPropagation(); el.classList.add('dragover'); });
        el.addEventListener('dragleave', function(e) { e.preventDefault(); el.classList.remove('dragover'); });
        el.addEventListener('drop', function(e) {
            e.preventDefault(); e.stopPropagation(); el.classList.remove('dragover');
            var file = e.dataTransfer.files[0];
            if (file) readFileIntoTextarea(textareaId, file);
        });
        el.addEventListener('click', function() {
            var input = document.createElement('input');
            input.type = 'file';
            input.accept = '.json,.yaml,.yml,.toml,.xml,.csv,.sql,.html,.md,.properties,.ini,.env,.txt';
            input.onchange = function(e) { if (e.target.files[0]) readFileIntoTextarea(textareaId, e.target.files[0]); };
            input.click();
        });
    }

    function readFileIntoTextarea(textareaId, file) {
        var reader = new FileReader();
        reader.onload = function(e) {
            document.getElementById(textareaId).value = e.target.result;
        };
        reader.readAsText(file);
    }

    var EXAMPLES = {
        'json-config': {
            driver: 'json',
            base: '{\n  "server": {\n    "host": "0.0.0.0",\n    "port": 8080,\n    "tls": false\n  },\n  "logging": {\n    "level": "info",\n    "format": "json"\n  }\n}',
            ours: '{\n  "server": {\n    "host": "0.0.0.0",\n    "port": 9090,\n    "tls": true,\n    "cert": "/etc/ssl/cert.pem"\n  },\n  "logging": {\n    "level": "info",\n    "format": "json"\n  }\n}',
            theirs: '{\n  "server": {\n    "host": "127.0.0.1",\n    "port": 8080,\n    "tls": false\n  },\n  "logging": {\n    "level": "debug",\n    "format": "text"\n  }\n}'
        },
        'json-package': {
            driver: 'json',
            base: '{\n  "name": "my-app",\n  "version": "1.0.0",\n  "dependencies": {\n    "express": "^4.18.0",\n    "lodash": "^4.17.0"\n  }\n}',
            ours: '{\n  "name": "my-app",\n  "version": "1.1.0",\n  "dependencies": {\n    "express": "^4.18.0",\n    "lodash": "^4.17.0",\n    "axios": "^1.5.0"\n  }\n}',
            theirs: '{\n  "name": "my-app",\n  "version": "1.0.0",\n  "dependencies": {\n    "express": "^4.19.0",\n    "lodash": "^4.17.21"\n  }\n}'
        },
        'yaml-k8s': {
            driver: 'yaml',
            base: 'apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\nspec:\n  replicas: 3\n  template:\n    spec:\n      containers:\n        - name: app\n          image: my-app:1.0\n          ports:\n            - containerPort: 8080',
            ours: 'apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\n  labels:\n    env: production\nspec:\n  replicas: 5\n  template:\n    spec:\n      containers:\n        - name: app\n          image: my-app:1.1\n          ports:\n            - containerPort: 8080',
            theirs: 'apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\n  annotations:\n    description: "Production deployment"\nspec:\n  replicas: 3\n  template:\n    spec:\n      containers:\n        - name: app\n          image: my-app:1.0\n          ports:\n            - containerPort: 8080\n            - containerPort: 9090'
        },
        'toml-cargo': {
            driver: 'toml',
            base: '[package]\nname = "suture"\nversion = "5.0.1"\nedition = "2021"\n\n[dependencies]\ntokio = { version = "1", features = ["full"] }',
            ours: '[package]\nname = "suture"\nversion = "5.1.0"\nedition = "2021"\n\n[dependencies]\ntokio = { version = "1", features = ["full"] }\nserde = { version = "1", features = ["derive"] }',
            theirs: '[package]\nname = "suture"\nversion = "5.0.1"\nedition = "2021"\n\n[dependencies]\ntokio = { version = "1.2", features = ["full"] }\n\n[profile.release]\nopt-level = 3'
        },
        'csv-data': {
            driver: 'csv',
            base: 'id,name,email,role\n1,Alice,alice@example.com,admin\n2,Bob,bob@example.com,user\n3,Carol,carol@example.com,user',
            ours: 'id,name,email,role\n1,Alice,alice@example.com,admin\n2,Bob,bob@newdomain.com,user\n4,Dave,dave@example.com,user',
            theirs: 'id,name,email,role\n1,Alice,alice@example.com,owner\n2,Bob,bob@example.com,user\n3,Carol,carol@example.com,moderator'
        }
    };

    function loadExample() {
        var key = document.getElementById('example-select').value;
        if (!key || !EXAMPLES[key]) return;
        var ex = EXAMPLES[key];
        document.getElementById('merge-driver').value = ex.driver;
        document.getElementById('merge-base').value = ex.base;
        document.getElementById('merge-ours').value = ex.ours;
        document.getElementById('merge-theirs').value = ex.theirs;
        showDriverOptions();
    }

    function showDriverOptions() {
        var driver = document.getElementById('merge-driver').value;
        var el = document.getElementById('driver-options');
        if (!el) return;
        var opts = {
            json: '<div class="form-group"><label>Indentation</label><select><option>2 spaces</option><option>4 spaces</option><option>Tab</option></select></div>',
            yaml: '<div class="form-group"><label>YAML Version</label><select><option>1.2</option><option>1.1</option></select></div>',
            csv: '<div class="form-row"><div class="form-group"><label>Delimiter</label><select><option>,</option><option>;</option><option>\t</option></select></div><div class="form-group"><label>Has Header</label><select><option>Yes</option><option>No</option></select></div></div>',
            xml: '<div class="form-group"><label>Pretty Print</label><select><option>Yes</option><option>No</option></select></div>'
        };
        if (opts[driver]) {
            el.innerHTML = '<div class="card" style="padding:1rem"><h3 style="font-size:0.85rem;margin-bottom:0.5rem">' + driver.toUpperCase() + ' Options</h3>' + opts[driver] + '</div>';
            el.classList.remove('hidden');
        } else {
            el.classList.add('hidden');
        }
    }

    function getMergeParams() {
        var hash = location.hash;
        var qIdx = hash.indexOf('?');
        if (qIdx < 0) return {};
        var search = hash.substring(qIdx + 1);
        var p = new URLSearchParams(search);
        var r = { driver: p.get('driver'), base: p.get('base'), ours: p.get('ours'), theirs: p.get('theirs') };
        try {
            if (r.base) r.base = decodeURIComponent(escape(atob(r.base)));
            if (r.ours) r.ours = decodeURIComponent(escape(atob(r.ours)));
            if (r.theirs) r.theirs = decodeURIComponent(escape(atob(r.theirs)));
        } catch(e) {}
        return r;
    }

    function shareMerge() {
        var base = document.getElementById('merge-base').value;
        var ours = document.getElementById('merge-ours').value;
        var theirs = document.getElementById('merge-theirs').value;
        var driver = document.getElementById('merge-driver').value;
        var b64 = btoa(unescape(encodeURIComponent(base)));
        var o64 = btoa(unescape(encodeURIComponent(ours)));
        var t64 = btoa(unescape(encodeURIComponent(theirs)));
        var url = location.origin + location.pathname + '#/merge?driver=' + encodeURIComponent(driver) + '&base=' + encodeURIComponent(b64) + '&ours=' + encodeURIComponent(o64) + '&theirs=' + encodeURIComponent(t64);
        navigator.clipboard.writeText(url).then(function() { showToast('Share link copied to clipboard!'); });
    }

    function showToast(msg) {
        var toast = document.getElementById('share-toast');
        if (!toast) {
            toast = document.createElement('div');
            toast.id = 'share-toast';
            toast.className = 'share-toast';
            document.body.appendChild(toast);
        }
        toast.textContent = msg;
        toast.classList.add('show');
        setTimeout(function() { toast.classList.remove('show'); }, 2000);
    }

    function mergeObjects(base, ours, theirs) {
        var conflicts = 0;
        var resolved = 0;
        function mergeValue(b, o, t) {
            if (typeof o === 'object' && o !== null && !Array.isArray(o) &&
                typeof t === 'object' && t !== null && !Array.isArray(t) &&
                typeof b === 'object' && b !== null && !Array.isArray(b)) {
                var result = {};
                var allKeys = Object.keys(b).concat(Object.keys(o)).concat(Object.keys(t));
                var seen = {};
                allKeys.forEach(function(key) {
                    if (seen[key]) return;
                    seen[key] = true;
                    result[key] = mergeValue(
                        b.hasOwnProperty(key) ? b[key] : undefined,
                        o.hasOwnProperty(key) ? o[key] : undefined,
                        t.hasOwnProperty(key) ? t[key] : undefined
                    );
                });
                return result;
            }
            if (Array.isArray(o) && Array.isArray(t)) {
                var combined = o.slice();
                t.forEach(function(item) {
                    var found = false;
                    for (var i = 0; i < o.length; i++) {
                        if (JSON.stringify(o[i]) === JSON.stringify(item)) { found = true; break; }
                    }
                    if (!found) combined.push(item);
                });
                resolved++;
                return combined;
            }
            var oStr = JSON.stringify(o);
            var tStr = JSON.stringify(t);
            var bStr = JSON.stringify(b);
            if (oStr === tStr) return o;
            if (oStr === bStr) { resolved++; return t; }
            if (tStr === bStr) { resolved++; return o; }
            conflicts++;
            return { __suture_conflict: { ours: o, theirs: t } };
        }
        var result = mergeValue(base, ours, theirs);
        return { result: result, conflicts: conflicts, resolved: resolved };
    }

    function mergeJson(baseStr, oursStr, theirsStr) {
        return mergeObjects(JSON.parse(baseStr), JSON.parse(oursStr), JSON.parse(theirsStr));
    }

    function mergeYaml(baseStr, oursStr, theirsStr) {
        var conflicts = 0;
        var resolved = 0;
        function extractKeys(text) {
            var lines = text.split('\n');
            var map = {};
            var order = [];
            var path = [];
            lines.forEach(function(line) {
                var indent = 0;
                while (indent < line.length && line[indent] === ' ') indent++;
                var trimmed = line.trim();
                if (!trimmed || trimmed.indexOf('#') === 0) return;
                var level = indent / 2;
                while (path.length > level) path.pop();
                if (trimmed.indexOf('- ') === 0) {
                    var itemContent = trimmed.substring(2).trim();
                    path.push('-');
                    var fullPath = path.join('.') + ':' + itemContent;
                    map[fullPath] = line;
                    order.push(fullPath);
                    path.pop();
                    var ic = itemContent.indexOf(':');
                    if (ic > 0 && itemContent.substring(ic + 1).trim() === '') {
                        path.push(itemContent.substring(0, ic).trim());
                    }
                } else {
                    var ci = trimmed.indexOf(':');
                    if (ci > 0) {
                        var key = trimmed.substring(0, ci).trim();
                        var val = trimmed.substring(ci + 1).trim();
                        path.push(key);
                        var fp = path.join('.');
                        map[fp] = line;
                        order.push(fp);
                        if (val === '' || val === '|' || val === '>') {
                        } else {
                            path.pop();
                        }
                    }
                }
            });
            return { map: map, order: order };
        }
        var bp = extractKeys(baseStr), op = extractKeys(oursStr), tp = extractKeys(theirsStr);
        var allKeys = bp.order.concat(op.order).concat(tp.order);
        var seen = {};
        var resultLines = [];
        allKeys.forEach(function(key) {
            if (seen[key]) return;
            seen[key] = true;
            var bv = bp.map[key], ov = op.map[key], tv = tp.map[key];
            if (ov === tv) { resultLines.push(ov); }
            else if (ov === bv) { resolved++; resultLines.push(tv); }
            else if (tv === bv) { resolved++; resultLines.push(ov); }
            else { conflicts++; resultLines.push(ov + '  # << CONFLICT: ' + tv.trim()); }
        });
        return { formatted: resultLines.join('\n'), conflicts: conflicts, resolved: resolved };
    }

    function mergeToml(baseStr, oursStr, theirsStr) {
        var conflicts = 0;
        var resolved = 0;
        function parseToml(text) {
            var sections = {};
            var sectionOrder = [];
            var currentSection = null;
            text.split('\n').forEach(function(line) {
                var trimmed = line.trim();
                if (!trimmed || trimmed.indexOf('#') === 0) return;
                var m = trimmed.match(/^\[(.+)\]$/);
                if (m) {
                    currentSection = m[1];
                    if (!sections[currentSection]) { sections[currentSection] = {}; sectionOrder.push(currentSection); }
                    return;
                }
                var eqIdx = trimmed.indexOf('=');
                if (eqIdx > 0 && currentSection) {
                    var key = trimmed.substring(0, eqIdx).trim();
                    sections[currentSection][key] = trimmed;
                }
            });
            return { sections: sections, sectionOrder: sectionOrder };
        }
        function mergeSection(bS, oS, tS) {
            var lines = [];
            var allKeys = Object.keys(bS).concat(Object.keys(oS)).concat(Object.keys(tS));
            var seen = {};
            allKeys.forEach(function(key) {
                if (seen[key]) return;
                seen[key] = true;
                var bv = bS[key], ov = oS[key], tv = tS[key];
                if (ov === tv) { lines.push(ov); }
                else if (ov === bv) { resolved++; lines.push(tv); }
                else if (tv === bv) { resolved++; lines.push(ov); }
                else { conflicts++; lines.push(ov + '  # CONFLICT: ' + tv.replace(/^[^=]+=\s*/, '')); }
            });
            return lines;
        }
        var bp = parseToml(baseStr), op = parseToml(oursStr), tp = parseToml(theirsStr);
        var allSections = bp.sectionOrder.concat(op.sectionOrder).concat(tp.sectionOrder);
        var seenS = {};
        var resultLines = [];
        allSections.forEach(function(sec) {
            if (seenS[sec]) return;
            seenS[sec] = true;
            resultLines.push('[' + sec + ']');
            resultLines = resultLines.concat(mergeSection(bp.sections[sec] || {}, op.sections[sec] || {}, tp.sections[sec] || {}));
        });
        return { formatted: resultLines.join('\n'), conflicts: conflicts, resolved: resolved };
    }

    function mergeCsv(baseStr, oursStr, theirsStr) {
        var conflicts = 0;
        var resolved = 0;
        function parseCsv(text) {
            var lines = text.trim().split('\n');
            if (lines.length === 0) return { header: '', rows: {} };
            var header = lines[0];
            var rows = {};
            for (var i = 1; i < lines.length; i++) {
                var cols = lines[i].split(',');
                if (cols.length > 0 && cols[0]) rows[cols[0].trim()] = lines[i];
            }
            return { header: header, rows: rows };
        }
        var b = parseCsv(baseStr), o = parseCsv(oursStr), t = parseCsv(theirsStr);
        var allKeys = Object.keys(b.rows).concat(Object.keys(o.rows)).concat(Object.keys(t.rows));
        var seen = {};
        var resultLines = [b.header || o.header || t.header];
        allKeys.forEach(function(key) {
            if (seen[key]) return;
            seen[key] = true;
            var bv = b.rows[key], ov = o.rows[key], tv = t.rows[key];
            if (ov === tv) { resultLines.push(ov); }
            else if (ov === bv) { resolved++; resultLines.push(tv); }
            else if (tv === bv) { resolved++; resultLines.push(ov); }
            else { conflicts++; resultLines.push(ov + '  /* CONFLICT with: ' + tv + ' */'); }
        });
        return { formatted: resultLines.join('\n'), conflicts: conflicts, resolved: resolved };
    }

    function clientSideMerge(base, ours, theirs, driver) {
        if (driver === 'json') {
            var r = mergeJson(base, ours, theirs);
            r.formatted = JSON.stringify(r.result, null, 2);
            return r;
        }
        if (driver === 'yaml') return mergeYaml(base, ours, theirs);
        if (driver === 'toml') return mergeToml(base, ours, theirs);
        if (driver === 'csv') return mergeCsv(base, ours, theirs);
        return null;
    }

    function highlightMergeResult(formatted, driver, baseStr) {
        var lines = formatted.split('\n');
        var baseLines = baseStr.split('\n');
        if (driver === 'json') {
            try { baseLines = JSON.stringify(JSON.parse(baseStr), null, 2).split('\n'); } catch(e) {}
        }
        var html = '';
        lines.forEach(function(line) {
            var escaped = escHtml(line);
            if (line.indexOf('__suture_conflict') >= 0 || line.indexOf('CONFLICT') >= 0) {
                html += '<div class="merge-line-conflict">' + escaped + '</div>';
            } else if (baseLines.indexOf(line) < 0 && line.trim() !== '' && line.trim() !== '{' && line.trim() !== '}' && line.trim() !== '[' && line.trim() !== ']') {
                html += '<div class="merge-line-added">' + escaped + '</div>';
            } else {
                html += '<div>' + escaped + '</div>';
            }
        });
        return html;
    }

    function showMergeStats(conflicts, resolved, timeMs) {
        var el = document.getElementById('merge-stats');
        if (!el) return;
        el.classList.remove('hidden');
        el.innerHTML =
            '<div class="merge-stats-item"><div class="dot" style="background:var(--success)"></div>' + resolved + ' fields auto-resolved</div>' +
            (conflicts > 0 ? '<div class="merge-stats-item"><div class="dot" style="background:var(--danger)"></div>' + conflicts + ' conflict' + (conflicts > 1 ? 's' : '') + ' detected</div>' : '') +
            '<div class="merge-stats-item"><div class="dot" style="background:var(--text-muted)"></div>Merged in ' + timeMs + 'ms</div>';
    }

    async function doMerge() {
        var driver = document.getElementById('merge-driver').value;
        var base = document.getElementById('merge-base').value;
        var ours = document.getElementById('merge-ours').value;
        var theirs = document.getElementById('merge-theirs').value;
        var resultEl = document.getElementById('merge-result');
        var statusEl = document.getElementById('merge-status');
        var conflictBadge = document.getElementById('merge-conflict-badge');
        statusEl.textContent = 'Merging...';
        statusEl.className = 'badge badge-warning';
        if (conflictBadge) conflictBadge.classList.add('hidden');
        var startTime = performance.now();
        try {
            var clientResult = clientSideMerge(base, ours, theirs, driver);
            var elapsed = (performance.now() - startTime).toFixed(1);
            if (clientResult) {
                var formatted = clientResult.formatted || JSON.stringify(clientResult.result, null, 2);
                resultEl.innerHTML = highlightMergeResult(formatted, driver, base);
                showMergeStats(clientResult.conflicts, clientResult.resolved, elapsed);
                if (clientResult.conflicts > 0) {
                    statusEl.textContent = 'Merged with conflicts';
                    statusEl.className = 'badge badge-warning';
                    if (conflictBadge) {
                        conflictBadge.textContent = clientResult.conflicts + ' conflict' + (clientResult.conflicts > 1 ? 's' : '');
                        conflictBadge.classList.remove('hidden');
                    }
                } else {
                    statusEl.textContent = 'Merged';
                    statusEl.className = 'badge badge-success';
                }
                return;
            }
        } catch(e) {
        }
        try {
            var headers = { 'Content-Type': 'application/json' };
            if (APP.token) headers['Authorization'] = 'Bearer ' + APP.token;
            var resp = await fetch('/api/merge', {
                method: 'POST',
                headers: headers,
                body: JSON.stringify({ driver: driver, base: base, ours: ours, theirs: theirs })
            });
            var data = await resp.json();
            var elapsed2 = (performance.now() - startTime).toFixed(1);
            if (data.result) {
                resultEl.textContent = data.result;
                showMergeStats(0, '?', elapsed2);
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
            resultEl.textContent = 'Error: ' + e.message + '\n\nUse a supported format (JSON, YAML, TOML, CSV) for client-side merging, or ensure the platform server is running.';
            statusEl.textContent = 'Error';
            statusEl.className = 'badge badge-danger';
        }
    }

    function copyResult() {
        var text = document.getElementById('merge-result').textContent;
        navigator.clipboard.writeText(text).then(function() {
            var btn = event.target;
            var orig = btn.textContent;
            btn.textContent = 'Copied!';
            setTimeout(function() { btn.textContent = orig; }, 1500);
        });
    }

    function renderBilling(el) {
        var tier = APP.user ? APP.user.tier : 'free';
        var u = APP.usage || {};
        var plans = [
            { id: 'free', name: 'Free', price: '$0', period: '', merges: '100/mo', storage: '100 MB', repos: '5', features: ['5 core drivers', 'Community support'] },
            { id: 'pro', name: 'Pro', price: '$9', period: '/user/mo', merges: '10,000/mo', storage: '10 GB', repos: 'Unlimited', features: ['All 17+ drivers', '7-day audit log', 'Priority support'] },
            { id: 'enterprise', name: 'Enterprise', price: '$29', period: '/user/mo', merges: 'Unlimited', storage: '100 GB', repos: 'Unlimited', features: ['SAML/SSO', 'Unlimited audit log', '99.99% SLA', 'Dedicated support'] }
        ];

        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header"><h2>Billing & Plans</h2></div>' +
            '<div class="card" style="margin-bottom:2rem;display:flex;justify-content:space-between;align-items:center;flex-wrap:wrap;gap:1rem">' +
                '<div>' +
                    '<h3 style="margin-bottom:0.25rem">Current Plan: ' + tierBadge(tier) + '</h3>' +
                    '<p style="color:var(--text-muted);font-size:0.9rem">' +
                        'Period: ' + (u.period || 'N/A') + ' | ' +
                        'Merges: ' + (u.merges_used || 0) + '/' + (u.merges_limit === -1 ? '\u221E' : (u.merges_limit || 100)) + ' | ' +
                        'Storage: ' + formatBytes(u.storage_bytes || 0) + '/' + formatBytes(u.storage_limit || 100*1024*1024) +
                    '</p>' +
                '</div>' +
                '<div style="display:flex;gap:0.5rem">' +
                    (tier !== 'enterprise' ? '<button class="btn btn-primary" onclick="upgradePlan()">Upgrade Plan</button>' : '') +
                    '<button class="btn" onclick="manageSubscription()">Manage Subscription</button>' +
                '</div>' +
            '</div>' +
            '<h3 style="margin-bottom:1rem">Usage Breakdown</h3>' +
            '<div class="card" style="margin-bottom:2rem">' +
                '<table class="usage-table">' +
                '<thead><tr><th>Resource</th><th>Used</th><th>Limit</th><th>Usage</th></tr></thead>' +
                '<tbody>' +
                '<tr><td>Merges</td><td>' + (u.merges_used || 0) + '</td><td>' + (u.merges_limit === -1 ? '\u221E' : (u.merges_limit || 100)) + '</td><td><div class="usage-bar" style="width:100px;display:inline-block;vertical-align:middle"><div class="usage-bar-fill ' + barColor(u.merges_limit > 0 ? (u.merges_used/u.merges_limit)*100 : 0) + '" style="width:' + (u.merges_limit > 0 ? Math.min((u.merges_used/u.merges_limit)*100, 100) : 0) + '%"></div></div></td></tr>' +
                '<tr><td>Storage</td><td>' + formatBytes(u.storage_bytes || 0) + '</td><td>' + formatBytes(u.storage_limit || 100*1024*1024) + '</td><td><div class="usage-bar" style="width:100px;display:inline-block;vertical-align:middle"><div class="usage-bar-fill ' + barColor(u.storage_limit > 0 ? (u.storage_bytes/u.storage_limit)*100 : 0) + '" style="width:' + (u.storage_limit > 0 ? Math.min((u.storage_bytes/u.storage_limit)*100, 100) : 0) + '%"></div></div></td></tr>' +
                '<tr><td>Repositories</td><td>' + (u.repos_count || 0) + '</td><td>' + (u.repos_limit === -1 ? '\u221E' : (u.repos_limit || 5)) + '</td><td><div class="usage-bar" style="width:100px;display:inline-block;vertical-align:middle"><div class="usage-bar-fill ' + barColor(u.repos_limit > 0 ? (u.repos_count/u.repos_limit)*100 : 0) + '" style="width:' + (u.repos_limit > 0 ? Math.min((u.repos_count/u.repos_limit)*100, 100) : 0) + '%"></div></div></td></tr>' +
                '<tr><td>API Calls</td><td>' + (u.api_calls || 0) + '</td><td>\u221E</td><td>-</td></tr>' +
                '</tbody></table>' +
            '</div>' +
            '<h3 style="margin-bottom:1rem">Available Plans</h3>' +
            '<div class="grid grid-3">' +
            plans.map(function(p) {
                var isCurrent = tier === p.id;
                var cardClass = 'card pricing-card' + (p.id === 'pro' ? ' featured' : '') + (isCurrent ? ' current' : '');
                return '<div class="' + cardClass + '" style="text-align:center">' +
                    '<h3>' + p.name + (isCurrent ? ' <span class="badge badge-success">Current</span>' : '') + '</h3>' +
                    '<div class="plan-price">' + p.price + '<span>' + p.period + '</span></div>' +
                    '<ul style="list-style:none;margin:1rem 0;font-size:0.9rem;text-align:left">' +
                    '<li>\u2713 ' + p.merges + ' merges</li>' +
                    '<li>\u2713 ' + p.storage + ' storage</li>' +
                    '<li>\u2713 ' + p.repos + ' repositories</li>' +
                    p.features.map(function(f) { return '<li>\u2713 ' + f + '</li>'; }).join('') +
                    '</ul>' +
                    (isCurrent
                        ? '<button class="btn" style="width:100%;justify-content:center" disabled>Current Plan</button>'
                        : '<button class="btn btn-primary" style="width:100%;justify-content:center" onclick="selectPlan(\'' + p.id + '\')">' +
                          (planCompare(tier, p.id) > 0 ? 'Downgrade' : 'Upgrade') + ' to ' + p.name + '</button>'
                    ) +
                '</div>';
            }).join('') +
            '</div>' +
        '</div>';
    }

    function planCompare(current, target) {
        var order = { free: 0, pro: 1, enterprise: 2 };
        return (order[target] || 0) - (order[current] || 0);
    }

    function upgradePlan() {
        var tier = APP.user ? APP.user.tier : 'free';
        var next = tier === 'free' ? 'pro' : 'enterprise';
        selectPlan(next);
    }

    function selectPlan(planId) {
        if (!checkAuth()) { showAuth('register'); return; }
        fetch('/billing/checkout', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'Authorization': 'Bearer ' + APP.token },
            body: JSON.stringify({ tier: planId })
        })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            if (data.url) {
                window.location.href = data.url;
            } else {
                alert(data.error || 'Checkout is not configured. Contact support.');
            }
        })
        .catch(function(e) { alert('Error: ' + e.message); });
    }

    function manageSubscription() {
        if (!checkAuth()) { showAuth('login'); return; }
        fetch('/billing/portal', {
            method: 'POST',
            headers: { 'Authorization': 'Bearer ' + APP.token }
        })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            if (data.url) {
                window.location.href = data.url;
            } else {
                alert(data.error || 'Billing portal is not configured.');
            }
        })
        .catch(function(e) { alert('Error: ' + e.message); });
    }

    function renderAPI(el) {
        var endpoints = [
            { method: 'POST', path: '/auth/register', desc: 'Register a new account', body: '{"email": "user@example.com",\n "password": "securepass123"}', curl: 'curl -X POST https://suture.example.com/auth/register \\\n  -H "Content-Type: application/json" \\\n  -d \'{"email":"user@example.com","password":"securepass123"}\'', python: 'import requests\n\nresp = requests.post(\n    "https://suture.example.com/auth/register",\n    json={"email": "user@example.com", "password": "securepass123"}\n)\nprint(resp.json())', js: 'const resp = await fetch("/auth/register", {\n  method: "POST",\n  headers: { "Content-Type": "application/json" },\n  body: JSON.stringify({\n    email: "user@example.com",\n    password: "securepass123"\n  })\n});\nconst data = await resp.json();' },
            { method: 'POST', path: '/auth/login', desc: 'Login and get JWT token', body: '{"email": "user@example.com",\n "password": "securepass123"}', curl: 'curl -X POST https://suture.example.com/auth/login \\\n  -H "Content-Type: application/json" \\\n  -d \'{"email":"user@example.com","password":"securepass123"}\'', python: 'import requests\n\nresp = requests.post(\n    "https://suture.example.com/auth/login",\n    json={"email": "user@example.com", "password": "securepass123"}\n)\ntoken = resp.json()["token"]', js: 'const resp = await fetch("/auth/login", {\n  method: "POST",\n  headers: { "Content-Type": "application/json" },\n  body: JSON.stringify({\n    email: "user@example.com",\n    password: "securepass123"\n  })\n});\nconst { token } = await resp.json();' },
            { method: 'GET', path: '/auth/me', desc: 'Get current authenticated user info', body: null, curl: 'curl https://suture.example.com/auth/me \\\n  -H "Authorization: Bearer $TOKEN"', python: 'import requests\n\nresp = requests.get(\n    "https://suture.example.com/auth/me",\n    headers={"Authorization": f"Bearer {token}"}\n)\nprint(resp.json())', js: 'const resp = await fetch("/auth/me", {\n  headers: { "Authorization": "Bearer " + token }\n});\nconst user = await resp.json();' },
            { method: 'POST', path: '/api/merge', desc: '3-way semantic merge', body: '{"driver": "json",\n "base": "{\\"key\\": \\"base\\"}",\n "ours": "{\\"key\\": \\"ours\\"}",\n "theirs": "{\\"key\\": \\"theirs\\"}"}', curl: 'curl -X POST https://suture.example.com/api/merge \\\n  -H "Content-Type: application/json" \\\n  -H "Authorization: Bearer $TOKEN" \\\n  -d \'{"driver":"json","base":"{\\"key\\":\\"base\\"}","ours":"{\\"key\\":\\"ours\\"}","theirs":"{\\"key\\":\\"theirs\\"}"}\'', python: 'import requests\n\nresp = requests.post(\n    "https://suture.example.com/api/merge",\n    headers={"Authorization": f"Bearer {token}"},\n    json={\n        "driver": "json",\n        "base": \'{"key": "base"}\',\n        "ours": \'{"key": "ours"}\',\n        "theirs": \'{"key": "theirs"}\'\n    }\n)\nprint(resp.json()["result"])', js: 'const resp = await fetch("/api/merge", {\n  method: "POST",\n  headers: {\n    "Content-Type": "application/json",\n    "Authorization": "Bearer " + token\n  },\n  body: JSON.stringify({\n    driver: "json",\n    base: \'{"key": "base"}\',\n    ours: \'{"key": "ours"}\',\n    theirs: \'{"key": "theirs"}\'\n  })\n});\nconst data = await resp.json();' },
            { method: 'GET', path: '/api/drivers', desc: 'List supported merge drivers and file extensions', body: null, curl: 'curl https://suture.example.com/api/drivers \\\n  -H "Authorization: Bearer $TOKEN"', python: 'import requests\n\nresp = requests.get(\n    "https://suture.example.com/api/drivers",\n    headers={"Authorization": f"Bearer {token}"}\n)\nprint(resp.json())', js: 'const resp = await fetch("/api/drivers", {\n  headers: { "Authorization": "Bearer " + token }\n});\nconst drivers = await resp.json();' },
            { method: 'GET', path: '/api/usage', desc: 'Get current billing period usage and limits', body: null, curl: 'curl https://suture.example.com/api/usage \\\n  -H "Authorization: Bearer $TOKEN"', python: 'import requests\n\nresp = requests.get(\n    "https://suture.example.com/api/usage",\n    headers={"Authorization": f"Bearer {token}"}\n)\nprint(resp.json())', js: 'const resp = await fetch("/api/usage", {\n  headers: { "Authorization": "Bearer " + token }\n});\nconst usage = await resp.json();' }
        ];

        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header"><h2>API Documentation</h2></div>' +
            '<p style="color:var(--text-muted);margin-bottom:1.5rem">All authenticated endpoints accept <code>Authorization: Bearer &lt;token&gt;</code>. Get your token from <code>/auth/login</code>.</p>' +
            endpoints.map(function(ep, i) {
                var methodCls = 'api-method-' + ep.method.toLowerCase();
                return '<div class="api-endpoint" id="ep-' + i + '">' +
                    '<div class="api-endpoint-header" onclick="toggleEndpoint(' + i + ')">' +
                        '<span class="api-method ' + methodCls + '">' + ep.method + '</span>' +
                        '<span>' + ep.path + '</span>' +
                        '<span style="margin-left:auto;color:var(--text-muted);font-size:0.8rem">' + ep.desc + '</span>' +
                        '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:16px;height:16px;flex-shrink:0"><path d="M6 9l6 6 6-6"/></svg>' +
                    '</div>' +
                    '<div class="api-endpoint-body">' +
                        (ep.body ? '<p style="margin-bottom:0.5rem;font-size:0.8rem;color:var(--text-muted)">Request Body:</p><div class="code-block"><button class="btn btn-sm copy-btn" onclick="copyCode(this)">Copy</button>' + escHtml(ep.body) + '</div>' : '') +
                        '<div class="tab-bar" style="margin-top:1rem">' +
                            '<button class="active" onclick="switchTab(this, \'curl-' + i + '\')">cURL</button>' +
                            '<button onclick="switchTab(this, \'python-' + i + '\')">Python</button>' +
                            '<button onclick="switchTab(this, \'js-' + i + '\')">JavaScript</button>' +
                            (APP.token ? '<button onclick="switchTab(this, \'tryit-' + i + '\')">Try it</button>' : '') +
                        '</div>' +
                        '<div class="tab-content active" id="tab-curl-' + i + '"><div class="code-block"><button class="btn btn-sm copy-btn" onclick="copyCode(this)">Copy</button>' + escHtml(ep.curl) + '</div></div>' +
                        '<div class="tab-content" id="tab-python-' + i + '"><div class="code-block"><button class="btn btn-sm copy-btn" onclick="copyCode(this)">Copy</button>' + escHtml(ep.python) + '</div></div>' +
                        '<div class="tab-content" id="tab-js-' + i + '"><div class="code-block"><button class="btn btn-sm copy-btn" onclick="copyCode(this)">Copy</button>' + escHtml(ep.js) + '</div></div>' +
                        (APP.token ? '<div class="tab-content" id="tab-tryit-' + i + '"><div class="tryit-panel">' +
                            (ep.method === 'POST' && ep.body ? '<div class="form-group"><label>Request Body</label><textarea id="tryit-body-' + i + '" style="min-height:80px">' + escHtml(ep.body) + '</textarea></div>' : '') +
                            '<button class="btn btn-primary btn-sm" onclick="tryIt(' + i + ',\'' + ep.method + '\',\'' + ep.path + '\')">Send Request</button>' +
                            '<div class="tryit-response" id="tryit-resp-' + i + '"></div>' +
                        '</div></div>' : '') +
                    '</div>' +
                '</div>';
            }).join('') +
        '</div>';
    }

    function escHtml(str) {
        return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    function toggleEndpoint(i) {
        var el = document.getElementById('ep-' + i);
        if (el) el.classList.toggle('open');
    }

    function switchTab(btn, tabId) {
        var bar = btn.parentElement;
        bar.querySelectorAll('button').forEach(function(b) { b.classList.remove('active'); });
        btn.classList.add('active');
        var parent = bar.parentElement;
        parent.querySelectorAll('.tab-content').forEach(function(tc) { tc.classList.remove('active'); });
        var tab = document.getElementById('tab-' + tabId);
        if (tab) tab.classList.add('active');
    }

    function copyCode(btn) {
        var block = btn.parentElement;
        var text = block.textContent.replace('Copy', '').trim();
        navigator.clipboard.writeText(text).then(function() {
            var orig = btn.textContent;
            btn.textContent = 'Copied!';
            setTimeout(function() { btn.textContent = orig; }, 1500);
        });
    }

    async function tryIt(i, method, path) {
        var respEl = document.getElementById('tryit-resp-' + i);
        respEl.textContent = 'Sending...';
        try {
            var opts = { method: method, headers: {} };
            if (APP.token) opts.headers['Authorization'] = 'Bearer ' + APP.token;
            if (method === 'POST') {
                var bodyEl = document.getElementById('tryit-body-' + i);
                if (bodyEl) {
                    opts.headers['Content-Type'] = 'application/json';
                    opts.body = bodyEl.value;
                }
            }
            var resp = await fetch(path, opts);
            var data = await resp.json();
            respEl.textContent = 'Status: ' + resp.status + ' ' + resp.statusText + '\n\n' + JSON.stringify(data, null, 2);
        } catch(e) {
            respEl.textContent = 'Error: ' + e.message;
        }
    }

    function renderSettings(el) {
        el.innerHTML =
        '<div class="container">' +
            '<div class="page-header"><h2>Settings</h2></div>' +
            '<div class="grid grid-2">' +
                '<div>' +
                    '<div class="card settings-section">' +
                        '<h3>Account Information</h3>' +
                        '<div class="form-group"><label>Email</label><input type="email" value="' + (APP.user.email || '') + '" readonly style="opacity:0.7"></div>' +
                        '<div class="form-group"><label>Display Name</label><input type="text" value="' + (APP.user.display_name || '') + '" id="settings-name"></div>' +
                        '<div class="form-group"><label>Tier</label><input type="text" value="' + (APP.user.tier || 'free') + '" readonly style="opacity:0.7"></div>' +
                        '<div class="form-group"><label>Created</label><input type="text" value="' + (APP.user.created_at || 'N/A') + '" readonly style="opacity:0.7"></div>' +
                        '<button class="btn btn-primary btn-sm" onclick="saveDisplayName()">Save Changes</button>' +
                    '</div>' +
                    '<div class="card settings-section">' +
                        '<h3>Organizations</h3>' +
                        '<div id="orgs-list"></div>' +
                        '<div style="margin-top:1rem;padding-top:1rem;border-top:1px solid var(--border)">' +
                            '<h4 style="font-size:0.9rem;margin-bottom:0.5rem">Create Organization</h4>' +
                            '<div class="form-group"><input type="text" id="new-org-name" placeholder="Organization name"></div>' +
                            '<button class="btn btn-sm btn-primary" onclick="createOrg()">Create</button>' +
                        '</div>' +
                    '</div>' +
                '</div>' +
                '<div>' +
                    '<div class="card settings-section">' +
                        '<h3>Security</h3>' +
                        '<div class="form-group"><label>Change Password</label><input type="password" id="settings-old-pw" placeholder="Current password"></div>' +
                        '<div class="form-group"><label>New Password</label><input type="password" id="settings-new-pw" placeholder="New password (min 8 chars)"></div>' +
                        '<button class="btn btn-sm" onclick="changePassword()">Update Password</button>' +
                    '</div>' +
                    '<div class="card settings-section">' +
                        '<h3>API Token</h3>' +
                        '<p style="font-size:0.85rem;color:var(--text-muted);margin-bottom:0.5rem">Your current JWT token (stored in localStorage):</p>' +
                        '<div class="code-block" style="word-break:break-all;font-size:0.75rem">' + (APP.token ? APP.token.substring(0, 40) + '...' : 'Not authenticated') + '</div>' +
                        '<button class="btn btn-sm btn-ghost" style="margin-top:0.5rem" onclick="copyToken()">Copy Full Token</button>' +
                    '</div>' +
                    '<div class="danger-zone">' +
                        '<h3>Danger Zone</h3>' +
                        '<p style="font-size:0.9rem;margin-bottom:1rem">Permanently delete your account and all associated data. This action cannot be undone.</p>' +
                        '<button class="btn btn-sm btn-danger" onclick="confirmDeleteAccount()">Delete Account</button>' +
                    '</div>' +
                '</div>' +
            '</div>' +
        '</div>';

        loadOrgs();
    }

    async function loadOrgs() {
        var listEl = document.getElementById('orgs-list');
        if (!listEl) return;
        try {
            var data = await fetchJSON('/api/orgs');
            if (data.orgs && data.orgs.length > 0) {
                listEl.innerHTML = data.orgs.map(function(o) {
                    return '<div class="org-item"><div><div class="org-item-name">' + escHtml(o.name || o.org_id) + '</div><div class="org-item-role">' + (o.role || 'member') + '</div></div></div>';
                }).join('');
            } else {
                listEl.innerHTML = '<p style="font-size:0.85rem;color:var(--text-muted)">No organizations yet.</p>';
            }
        } catch(e) {
            listEl.innerHTML = '<p style="font-size:0.85rem;color:var(--text-muted)">Could not load organizations.</p>';
        }
    }

    async function createOrg() {
        var name = document.getElementById('new-org-name').value.trim();
        if (!name) { alert('Please enter an organization name.'); return; }
        try {
            var data = await fetchJSON('/api/orgs', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ name: name })
            });
            if (data.org_id) {
                document.getElementById('new-org-name').value = '';
                loadOrgs();
            } else {
                alert('Error: ' + (data.error || JSON.stringify(data)));
            }
        } catch(e) {
            alert('Error: ' + e.message);
        }
    }

    async function saveDisplayName() {
        var name = document.getElementById('settings-name').value.trim();
        try {
            var data = await fetchJSON('/auth/profile', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ display_name: name })
            });
            if (data.error) {
                alert('Error: ' + data.error);
            } else {
                APP.user.display_name = name;
                alert('Display name updated.');
            }
        } catch(e) {
            alert('Error: ' + e.message);
        }
    }

    async function changePassword() {
        var oldPw = document.getElementById('settings-old-pw').value;
        var newPw = document.getElementById('settings-new-pw').value;
        if (!oldPw || !newPw) { alert('Please fill in both fields.'); return; }
        if (newPw.length < 8) { alert('New password must be at least 8 characters.'); return; }
        try {
            var data = await fetchJSON('/auth/password', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ old_password: oldPw, new_password: newPw })
            });
            if (data.error) {
                alert('Error: ' + data.error);
            } else {
                alert('Password updated successfully.');
                document.getElementById('settings-old-pw').value = '';
                document.getElementById('settings-new-pw').value = '';
            }
        } catch(e) {
            alert('Error: ' + e.message);
        }
    }

    function copyToken() {
        if (APP.token) {
            navigator.clipboard.writeText(APP.token).then(function() { alert('Token copied to clipboard.'); });
        }
    }

    function confirmDeleteAccount() {
        var root = document.getElementById('modal-root');
        root.innerHTML = '<div class="modal-overlay" onclick="if(event.target===this)closeModal()">' +
            '<div class="modal">' +
            '<h3 style="color:var(--danger)">Delete Account</h3>' +
            '<p style="margin-bottom:1rem">This will permanently delete your account, all organizations you own, and all associated data. This cannot be undone.</p>' +
            '<div class="form-group"><label>Type your email to confirm</label><input type="email" id="delete-confirm-email" placeholder="' + (APP.user ? APP.user.email : '') + '"></div>' +
            '<div class="modal-actions">' +
            '<button class="btn" onclick="closeModal()">Cancel</button>' +
            '<button class="btn btn-danger" onclick="deleteAccount()">Delete Forever</button>' +
            '</div></div></div>';
    }

    async function deleteAccount() {
        var email = document.getElementById('delete-confirm-email').value.trim();
        if (email !== (APP.user ? APP.user.email : '')) {
            alert('Email does not match. Please type your email to confirm.');
            return;
        }
        try {
            var resp = await fetch('/auth/account', {
                method: 'DELETE',
                headers: { 'Authorization': 'Bearer ' + APP.token }
            });
            if (resp.ok) {
                closeModal();
                logout();
                alert('Account deleted successfully.');
            } else {
                var data = await resp.json();
                alert('Error: ' + (data.error || 'Could not delete account.'));
            }
        } catch(e) {
            alert('Error: ' + e.message);
        }
    }

    function toggleFaq(el) {
        el.classList.toggle('open');
    }

    function startMergePreview() {
        var codeEl = document.getElementById('merge-preview-code');
        if (!codeEl) return;
        var lines = [
            { text: '{', cls: 'diff-keep' },
            { text: '  "name": "suture",', cls: 'diff-keep' },
            { text: '  "version": "5.1.0",', cls: 'diff-add' },
            { text: '  "features": ["merge", "diff", "platform"],', cls: 'diff-add' },
            { text: '  "license": "AGPL-3.0"', cls: 'diff-add' },
            { text: '}', cls: 'diff-keep' }
        ];
        var step = 0;
        var currentText = '';
        function type() {
            if (step >= lines.length) {
                setTimeout(function() {
                    currentText = '';
                    step = 0;
                    codeEl.innerHTML = '';
                    type();
                }, 2500);
                return;
            }
            var line = lines[step];
            var idx = 0;
            var prefix = currentText ? currentText + '\n' : '';
            function typeChar() {
                if (idx >= line.text.length) {
                    currentText = line.text;
                    step++;
                    setTimeout(type, 120);
                    return;
                }
                idx++;
                var partial = prefix + '<span class="' + line.cls + '">' + line.text.substring(0, idx) + '</span>';
                codeEl.innerHTML = partial;
                setTimeout(typeChar, 30 + Math.random() * 40);
            }
            typeChar();
        }
        type();
    }

    window.addEventListener('hashchange', router);

    (async function init() {
        if (APP.token) {
            try {
                var data = await fetchJSON('/auth/me');
                if (data && data.user) {
                    APP.user = data.user;
                    updateNav();
                } else {
                    localStorage.removeItem('suture_token');
                    APP.token = null;
                }
            } catch(e) {}
        }
        if (APP.token) await loadUsage();
        router();
    })();
    </script>
</body>
</html>"##;
