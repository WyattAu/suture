const { existsSync } = require('fs');
const { join } = require('path');

let nativeBinding = null;

function loadBinding() {
  if (nativeBinding) return nativeBinding;

  const platform = process.platform;
  const arch = process.arch;
  const ext = platform === 'win32' ? '.dll' : platform === 'darwin' ? '.dylib' : '.so';
  const prefix = platform === 'win32' ? '' : 'lib';

  const candidates = [
    join(__dirname, `${prefix}suture.${ext}`),
    join(__dirname, '..', 'target', 'release', `${prefix}suture.${ext}`),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      nativeBinding = require(candidate);
      return nativeBinding;
    }
  }

  throw new Error('Could not find suture native binding. Run "npm run build" first.');
}

module.exports = new Proxy({}, {
  get(target, prop) {
    const binding = loadBinding();
    return binding[prop];
  }
});
