#!/usr/bin/env node
// download-binary.js — Downloads the suture binary for the current platform
// from GitHub Releases. Uses only Node.js built-in modules.

"use strict";

const { createWriteStream, mkdirSync, chmodSync, existsSync, statSync } = require("fs");
const { join, dirname } = require("path");
const { createGunzip } = require("zlib");
const { createHash } = require("crypto");
const { get } = require("https");
const { pipeline } = require("stream/promises");

const VERSION = "5.0.0";
const BASE_URL = `https://github.com/WyattAu/suture/releases/download/v${VERSION}`;

const PLATFORMS = {
  "linux-x64": "suture-linux-x64.tar.gz",
  "darwin-x64": "suture-darwin-x64.tar.gz",
  "darwin-arm64": "suture-darwin-arm64.tar.gz",
  "win32-x64": "suture-windows-x64.tar.gz",
};

function detectPlatform() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : null;
  if (!arch) {
    console.error(`Unsupported architecture: ${process.arch}`);
    process.exit(1);
  }
  const os = process.platform === "win32" ? "win32" : process.platform;
  const key = `${os}-${arch}`;
  if (!PLATFORMS[key]) {
    console.error(`Unsupported platform: ${key}`);
    process.exit(1);
  }
  return key;
}

function httpsGet(url) {
  return new Promise((resolve, reject) => {
    get(url, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        httpsGet(res.headers.location).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        return;
      }
      resolve(res);
    }).on("error", reject);
  });
}

async function download(url, dest) {
  const res = await httpsGet(url);
  await pipeline(res, createWriteStream(dest));
}

async function fetchText(url) {
  const res = await httpsGet(url);
  const chunks = [];
  for await (const chunk of res) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf-8");
}

async function extractTarGz(archivePath, destDir) {
  // Minimal tar.gz extraction using built-in modules.
  // Tar format: 512-byte headers followed by file data padded to 512-byte blocks.
  const zlib = require("zlib");
  const { createReadStream } = require("fs");
  const { pipeline } = require("stream/promises");

  const gunzip = createGunzip();
  const readStream = createReadStream(archivePath);
  const chunks = [];
  await pipeline(readStream, gunzip, async function* (source) {
    for await (const chunk of source) {
      chunks.push(chunk);
    }
  });
  const tarData = Buffer.concat(chunks);
  let offset = 0;

  while (offset < tarData.length - 512) {
    const header = tarData.subarray(offset, offset + 512);
    // Empty block — end of archive
    if (header.every((b) => b === 0)) break;

    const name = header.subarray(0, 100).toString("ascii").replace(/\0.*$/, "").trim();
    const sizeStr = header.subarray(124, 136).toString("ascii").replace(/\0.*$/, "").trim();
    const size = parseInt(sizeStr, 8) || 0;
    const typeFlag = String.fromCharCode(header[156]);
    offset += 512;

    if (size === 0 || typeFlag === "5" || name.endsWith("/")) {
      // Directory or empty — skip
      offset += Math.ceil(size / 512) * 512;
      continue;
    }

    const fileData = tarData.subarray(offset, offset + size);
    const outPath = join(destDir, name);
    mkdirSync(dirname(outPath), { recursive: true });
    require("fs").writeFileSync(outPath, fileData);
    if (process.platform !== "win32") {
      chmodSync(outPath, 0o755);
    }
    offset += Math.ceil(size / 512) * 512;
  }
}

async function verifySha256(filePath, expectedHash) {
  const { createReadStream, readFileSync } = require("fs");
  const hash = createHash("sha256");
  await pipeline(createReadStream(filePath), hash);
  const actual = hash.digest("hex").toLowerCase();
  if (actual !== expectedHash.toLowerCase()) {
    throw new Error(
      `SHA256 mismatch for ${filePath}\n  expected: ${expectedHash}\n  actual:   ${actual}`
    );
  }
}

async function main() {
  const pkgDir = dirname(require("url").pathToFileURL(__filename).pathname.replace(/^file:/, ""));
  const platform = detectPlatform();
  const archiveName = PLATFORMS[platform];
  const archiveUrl = `${BASE_URL}/${archiveName}`;
  const checksumsUrl = `${BASE_URL}/checksums.sha256`;
  const binDir = join(pkgDir, "binaries", platform);

  console.log(`suture-merge-driver: downloading suture ${VERSION} for ${platform}...`);

  mkdirSync(binDir, { recursive: true });
  const archivePath = join(binDir, archiveName);
  const binPath = join(binDir, "suture");

  // Already downloaded and executable
  if (existsSync(binPath) && statSync(binPath).size > 0) {
    console.log("suture-merge-driver: binary already exists, skipping download.");
    return;
  }

  // Download checksums first
  let expectedHash = "";
  try {
    const checksums = await fetchText(checksumsUrl);
    for (const line of checksums.split("\n")) {
      const match = line.trim().match(/^([a-fA-F0-9]+)\s+/);
      if (match && line.includes(archiveName)) {
        expectedHash = match[1];
        break;
      }
    }
  } catch {
    console.warn("Warning: could not fetch checksums, skipping verification.");
  }

  await download(archiveUrl, archivePath);

  if (expectedHash) {
    await verifySha256(archivePath, expectedHash);
    console.log("Checksum verified.");
  }

  await extractTarGz(archivePath, binDir);

  // Clean up archive
  require("fs").unlinkSync(archivePath);

  console.log("suture-merge-driver: installed successfully.");
}

main().catch((err) => {
  console.error(`suture-merge-driver: download failed — ${err.message}`);
  process.exit(1);
});
