const core = require('@actions/core');
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

async function fetchMerge(apiUrl, apiToken, driver, base, ours, theirs) {
  const url = `${apiUrl}/merge`;
  const headers = { 'Content-Type': 'application/json' };
  if (apiToken) headers['Authorization'] = `Bearer ${apiToken}`;

  const resp = await fetch(url, {
    method: 'POST',
    headers,
    body: JSON.stringify({ driver, base, ours, theirs }),
  });
  const data = await resp.json();
  return { ok: resp.ok, status: resp.status, ...data };
}

function getFileAtRef(filePath, ref) {
  try {
    return execFileSync('git', ['show', `${ref}:${filePath}`], { encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 });
  } catch {
    return null;
  }
}

function getDriverForFile(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  const map = {
    '.json': 'json', '.yaml': 'yaml', '.yml': 'yaml',
    '.toml': 'toml', '.xml': 'xml', '.csv': 'csv',
    '.sql': 'sql', '.html': 'html', '.htm': 'html',
    '.md': 'markdown', '.svg': 'svg',
    '.properties': 'properties', '.ini': 'properties',
  };
  return map[ext] || null;
}

async function run() {
  try {
    const filesInput = core.getInput('files', { required: true });
    const apiUrl = core.getInput('api-url') || 'https://merge.suture.dev/api';
    const apiToken = core.getInput('api-token');
    const baseRef = core.getInput('base-ref') || 'HEAD~1';
    const oursRef = core.getInput('ours-ref') || 'HEAD';
    const theirsRef = core.getInput('theirs-ref');
    const failOnConflict = core.getInput('fail-on-conflict') !== 'false';
    const createComment = core.getInput('create-comment') === 'true';

    const files = filesInput.split('\n').map(f => f.trim()).filter(Boolean);
    const results = [];
    let conflictFiles = [];

    for (const file of files) {
      const driver = core.getInput('driver') || getDriverForFile(file);
      if (!driver) {
        core.warning(`Unknown driver for ${file}, skipping`);
        continue;
      }

      const base = getFileAtRef(file, baseRef);
      const ours = getFileAtRef(file, oursRef);
      const theirs = theirsRef ? getFileAtRef(file, theirsRef) : ours;

      if (base === null && ours === null) {
        core.warning(`File ${file} not found at any ref, skipping`);
        continue;
      }

      const effectiveBase = base || ours || '';
      const effectiveOurs = ours || '';
      const effectiveTheirs = theirs || ours || '';

      core.info(`Merging ${file} with driver=${driver}`);

      const result = await fetchMerge(apiUrl, apiToken, driver, effectiveBase, effectiveOurs, effectiveTheirs);

      if (result.result) {
        fs.writeFileSync(file, result.result);
        core.info(`Successfully merged ${file}`);
        results.push({ file, status: 'merged' });
      } else {
        core.warning(`Conflicts in ${file}`);
        conflictFiles.push(file);
        results.push({ file, status: 'conflict' });
      }
    }

    core.setOutput('merged-files', results.filter(r => r.status === 'merged').map(r => r.file).join('\n'));
    core.setOutput('conflict-files', conflictFiles.join('\n'));
    core.setOutput('has-conflicts', conflictFiles.length > 0 ? 'true' : 'false');
    core.setOutput('conflict-count', String(conflictFiles.length));

    if (failOnConflict && conflictFiles.length > 0) {
      core.setFailed(`Merge conflicts in: ${conflictFiles.join(', ')}`);
    }
  } catch (error) {
    core.setFailed(`Action failed: ${error.message}`);
  }
}

run();
