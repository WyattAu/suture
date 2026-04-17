/**
 * @suture/core — Node.js bindings for the Suture version control system.
 *
 * This module provides a native Node.js addon wrapping the Suture core
 * library, enabling JavaScript/TypeScript applications to use Suture's
 * patch-based version control with semantic merge capabilities.
 *
 * @module @suture/core
 */

const path = require("path");

// Load the native addon
let native;
try {
  // Try loading from the standard cargo build output
  const addonPath = path.join(
    __dirname,
    "..",
    "..",
    "target",
    "release",
    `libsuture_node${process.platform === "win32" ? ".dll" : process.platform === "darwin" ? ".dylib" : ".so"}`
  );
  native = require(addonPath);
} catch {
  // Fallback: try loading from debug build
  try {
    const debugPath = path.join(
      __dirname,
      "..",
      "..",
      "target",
      "debug",
      `libsuture_node${process.platform === "win32" ? ".dll" : process.platform === "darwin" ? ".dylib" : ".so"}`
    );
    native = require(debugPath);
  } catch {
    native = null;
  }
}

if (!native) {
  console.warn(
    "[@suture/core] Native addon not found. Run 'cargo build -p suture-node --release' first."
  );
}

/**
 * Repository represents a Suture version-controlled directory.
 */
class Repository {
  /**
   * @param {string} repoPath Path to the repository
   */
  constructor(repoPath) {
    this._path = repoPath;
    this._native = native;
  }

  /**
   * Initialize a new Suture repository at the given path.
   * @param {string} repoPath Path to the directory
   * @param {string} author Author name
   * @returns {Promise<Repository>}
   */
  static async init(repoPath, author = "node") {
    if (!native) throw new Error("Native addon not loaded");
    native.initRepo(repoPath, author);
    return new Repository(repoPath);
  }

  /**
   * Open an existing Suture repository.
   * @param {string} repoPath Path to the repository
   * @returns {Promise<Repository>}
   */
  static async open(repoPath) {
    if (!native) throw new Error("Native addon not loaded");
    return new Repository(repoPath);
  }

  /**
   * Get the current branch name.
   * @returns {Promise<string>}
   */
  async currentBranch() {
    if (!native) throw new Error("Native addon not loaded");
    return native.getCurrentBranch(this._path);
  }

  /**
   * List all branches.
   * @returns {Promise<string[]>}
   */
  async listBranches() {
    if (!native) throw new Error("Native addon not loaded");
    return native.listBranches(this._path);
  }

  /**
   * Create a new branch.
   * @param {string} name Branch name
   * @returns {Promise<void>}
   */
  async createBranch(name) {
    if (!native) throw new Error("Native addon not loaded");
    native.createBranch(this._path, name);
  }

  /**
   * Add a file to the staging area.
   * @param {string} filePath Relative file path
   * @returns {Promise<void>}
   */
  async add(filePath) {
    if (!native) throw new Error("Native addon not loaded");
    native.addFile(this._path, filePath);
  }

  /**
   * Commit staged changes.
   * @param {string} message Commit message
   * @returns {Promise<string>} Commit hash
   */
  async commit(message) {
    if (!native) throw new Error("Native addon not loaded");
    return native.commit(this._path, message);
  }

  /**
   * Get the commit log.
   * @param {number} [limit=20] Maximum number of commits
   * @returns {Promise<Array<{hash: string, message: string, author: string, timestamp: number}>>}
   */
  async log(limit = 20) {
    if (!native) throw new Error("Native addon not loaded");
    return native.getLog(this._path, limit);
  }

  /**
   * Get repository status.
   * @returns {Promise<{branch: string, staged: string[], unstaged: string[], untracked: string[]}>}
   */
  async status() {
    if (!native) throw new Error("Native addon not loaded");
    return native.getStatus(this._path);
  }

  /**
   * Get the repository path.
   * @returns {string}
   */
  get path() {
    return this._path;
  }
}

module.exports = {
  Repository,
  version: native?.getVersion?.() ?? "0.0.0-unknown",
  /** @type {boolean} Whether the native addon loaded successfully */
  isNativeLoaded: native !== null,
};
