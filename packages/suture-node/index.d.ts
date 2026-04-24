export interface RepoInfo {
  path: string;
  head_branch: string | null;
  patch_count: number;
  branch_count: number;
}

export interface CommitResult {
  id: string;
  short_id: string;
}

export interface FileEntry {
  path: string;
  status: 'added' | 'modified' | 'deleted' | 'clean' | 'untracked';
}

export interface StatusResult {
  head_branch: string | null;
  patch_count: number;
  branch_count: number;
  staged_files: FileEntry[];
}

export interface LogEntry {
  id: string;
  short_id: string;
  author: string;
  message: string;
  timestamp: number;
  is_merge: boolean;
}

export interface BranchEntry {
  name: string;
  target: string;
  is_current: boolean;
}

export function init(path: string, author: string): RepoInfo;
export function open(path: string): RepoInfo;
export function status(path: string): StatusResult;
export function add(path: string, file: string): void;
export function addAll(path: string): number;
export function commit(path: string, message: string): CommitResult;
export function log(path: string, limit?: number): LogEntry[];
export function branches(path: string): BranchEntry[];
export function createBranch(path: string, name: string): void;
export function mergeJson(base: string, ours: string, theirs: string): string;
export function mergeYaml(base: string, ours: string, theirs: string): string;
export function mergeToml(base: string, ours: string, theirs: string): string;
export function mergeCsv(base: string, ours: string, theirs: string): string;
export function getVersion(): string;
