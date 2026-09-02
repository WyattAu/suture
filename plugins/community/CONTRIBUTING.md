# Contributing Plugins

## Submission Requirements

Before submitting a plugin to the registry, ensure the following:

1. **Validation**: The plugin passes `suture plugin validate` without errors.
2. **Tests**: Include a test suite covering expected merge behavior, edge cases, and conflict scenarios. Tests must pass under the current Suture version.
3. **Documentation**: Document which file formats are supported and any format-specific behavior (e.g., how anchors are handled in YAML, or array merge strategies in JSON).
4. **WASM Binary**: Provide a stable URL for the compiled `.wasm` binary.
5. **Checksum**: Include the SHA-256 checksum of the WASM binary in the registry entry.

## ABI Compliance Checklist

- [ ] Targets WASM ABI v2 (current version)
- [ ] Exports the required merge function with correct signature
- [ ] Handles memory allocation per ABI v2 conventions
- [ ] Returns structured error results on failure
- [ ] Does not use forbidden host imports

## Review Process

1. Open a pull request adding your `[[plugin]]` entry to `plugins/registry.toml`.
2. A maintainer will run `suture plugin validate` against your WASM binary.
3. The plugin's metadata, checksum, and documentation will be reviewed.
4. Once approved, the entry is merged and the `verified` flag is set to `true`.

Maintainers may request changes to metadata, documentation, or test coverage before approving a submission.
