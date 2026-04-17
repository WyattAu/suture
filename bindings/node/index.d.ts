/**
 * @suture/core — TypeScript declarations for Suture Node.js bindings
 */

/** A Suture version-controlled repository */
export declare class Repository {
  /** Path to the repository root */
  readonly path: string;

  /** Initialize a new Suture repository */
  static init(repoPath: string, author?: string): Promise<Repository>;

  /** Open an existing Suture repository */
  static open(repoPath: string): Promise<Repository>;

  /** Get the current branch name */
  currentBranch(): Promise<string>;

  /** List all branches */
  listBranches(): Promise<string[]>;

  /** Create a new branch */
  createBranch(name: string): Promise<void>;

  /** Add a file to the staging area */
  add(filePath: string): Promise<void>;

  /** Commit staged changes */
  commit(message: string): Promise<string>;

  /** Get the commit log */
  log(limit?: number): Promise<Array<{
    hash: string;
    message: string;
    author: string;
    timestamp: number;
  }>>;

  /** Get repository status */
  status(): Promise<{
    branch: string;
    staged: string[];
    unstaged: string[];
    untracked: string[];
  }>;
}

/** Library version string */
export const version: string;

/** Whether the native addon loaded successfully */
export const isNativeLoaded: boolean;
