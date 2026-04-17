# suture-py

Python bindings for [Suture](https://github.com/WyattAu/suture), a content-addressable version control system.

## Installation

```bash
pip install maturin
maturin develop --release
```

## Usage

```python
import suture

# Initialize a new repository
repo = suture.SutureRepo.init("/path/to/repo", author="Alice")

# Or open an existing one
repo = suture.SutureRepo.open("/path/to/repo")

# Check status
status = repo.status()
print(status.head_branch, status.patch_count)

# Stage and commit
repo.add("file.txt")
patch_id = repo.commit("Initial commit")
print(patch_id)

# View log
for entry in repo.log(None):
    print(f"{entry.id[:12]} {entry.author}: {entry.message}")
```

## Utilities

```python
# Hash data
hex_hash = suture.hash_bytes(b"hello world")

# Check if a directory is a Suture repo
if suture.is_repo("/path"):
    ...
```
