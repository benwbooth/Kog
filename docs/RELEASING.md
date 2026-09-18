# Releasing Kog

The release job (`Cross-platform packages`) builds every platform from a tag.
The steps below exist because each one has broken a release at least once.

## 1. Land the changes

Keep `main` building. The nix package build runs `cargo test`, so the suite
must be green before tagging.

## 2. Bump every workspace member's version

Kog is a workspace (`kog`, `crates/kog-core`, `crates/kog-audio`). Bump all of
them plus the metainfo release list:

```sh
sed -i 's/^version = "X.Y.Z"/version = "X.Y.(Z+1)"/' \
  Cargo.toml crates/kog-core/Cargo.toml crates/kog-audio/Cargo.toml
# add <release version="X.Y.(Z+1)" date="..."/> to
# packaging/linux/org.kog.player.metainfo.xml
```

## 3. Sync the lockfile — do not hand-edit it

`Cargo.lock` records each member's version. Rewriting only the first match
leaves the other members stale, and **every** CI job then dies with
`cannot update the lock file ... because --locked was passed`. Always let
cargo do it:

```sh
cargo update --workspace --offline
```

Verify all three members moved:

```sh
grep -A1 'name = "kog"$\|name = "kog-core"\|name = "kog-audio"' Cargo.lock | grep version
```

## 4. Gate the release cheaply

```sh
cargo check --locked --workspace
```

This catches the lockfile class of breakage in seconds. Do **not** gate with
`cargo build --release`: it recompiles the native helper set and the final
binary (several minutes) and duplicates the work the nix prebuild and CI
already do.

Only run a full `cargo build --release --locked` when the release specifically
changes native build scripts or linker inputs.

## 5. Commit, tag, dispatch

Tags carry `[skip ci]`, so the workflow must be dispatched manually:

```sh
git commit -m "Release Kog X.Y.Z [skip ci]"
git tag vX.Y.Z && git push origin main && git push origin vX.Y.Z
gh workflow run "Cross-platform packages" --ref vX.Y.Z
```

## 6. Update the flake pin and prebuild

```sh
# /etc/nixos/flake.nix: inputs.kog.url = "...?ref=refs/tags/vX.Y.Z&submodules=1"
nix flake lock --update-input kog
nixos-rebuild build --flake /etc/nixos#nixos
```

Then run `switch` **and fully restart Kog**; switching alone leaves the old
binary running.

## Pitfalls

- **Never commit a temporary `fetchGit` pin.** Verifying a packaged build from
  a dirty worktree needs one (cleanSource drops submodules), but leaving it
  committed freezes the package source at that revision forever.
- **Regenerate the Flatpak vendor list when `Cargo.lock` gains crates**:
  `uv run flatpak-cargo-generator.py Cargo.lock --output packaging/linux/cargo-sources.json`.
  The Flatpak job fails if it is stale. Path-only workspace members add
  nothing, so the split itself needed no change.
- **Publish flakiness:** large asset uploads occasionally return HTTP 4xx/5xx.
  The publish step retries each file individually; a genuine failure is
  re-runnable without rebuilding.
- **Local `nix build` from a dirty worktree fails on submodules** (cleanSource
  omits them). That is expected; the tag-based build materializes them.
