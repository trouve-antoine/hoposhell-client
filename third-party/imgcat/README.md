# imgcat pin

`hopo populate` downloads `imgcat` from a fixed upstream commit and checks its
SHA-256 against [`CHECKSUM`](CHECKSUM) before installing it.

- Upstream: https://github.com/gnachman/iterm2-shell-integration
- File: `utilities/imgcat`
- Commit: `db8a601d8037c6ce2296c4d64765869f13a5eb5a`
- Commit date: 2026-01-11
- Checked: 2026-11-15
- Size: 8,621 bytes
- Pinned URL: https://raw.githubusercontent.com/gnachman/iterm2-shell-integration/db8a601d8037c6ce2296c4d64765869f13a5eb5a/utilities/imgcat

At the time of pinning, the pinned file and
`https://iterm2.com/utilities/imgcat` had identical SHA-256 checksums.
The script does not declare a release version; the commit identifies it.

To update, review the upstream changes, select a new immutable commit, and
update both the URL in `src/populate.rs` and this checksum/provenance together.
Do not fetch an expected checksum dynamically during installation.

To check a separately downloaded copy named `imgcat`, run from its directory:

```sh
# Linux
sha256sum --check /path/to/hoposhell-client/third-party/imgcat/CHECKSUM

# macOS
shasum -a 256 --check /path/to/hoposhell-client/third-party/imgcat/CHECKSUM
```
