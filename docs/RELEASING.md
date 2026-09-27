# Releasing Kog

**Cross-platform packages** builds Linux, Flatpak, Windows, macOS, Android, and
iOS in one workflow. A successful version-tag build publishes all their assets
to the matching GitHub release, including separate Linux/macOS TUI and server
archives, the Android development APK, and the unsigned iOS IPA.

## Prepare the version

1. Keep `main` building and resolve any failing platform jobs.
2. Update the version in the root `Cargo.toml` and every `crates/*/Cargo.toml`,
   including the standalone web crate.
3. Update Android's `versionName` and increment `versionCode`; update iOS's
   `CFBundleShortVersionString` and increment `CFBundleVersion`.
4. Add the version and date to `packaging/linux/org.kog.player.metainfo.xml`.
   Update the pinned installation examples in `README.md` and `packaging/README.md`.
5. Refresh both lockfiles with Cargo, without changing dependency versions:

   ```sh
   cargo update --workspace --offline
   cargo update --workspace --offline --manifest-path crates/kog-web/Cargo.toml
   cargo check --locked --workspace
   ```

The root workspace and web crate have separate lockfiles. Every path package's
recorded version must match its manifest or `--locked` builds fail.

## Publish

Commit the version changes and push an annotated tag:

```sh
git commit -m "Release Kog X.Y.Z"
git tag -a vX.Y.Z -m "Kog X.Y.Z"
git push --atomic origin main vX.Y.Z
```

The tag automatically starts the release workflow. For a commit marked
`[skip ci]`, explicitly dispatch it with `gh workflow run packages.yml --ref
vX.Y.Z`. Leave `source_ref` empty and `mobile_only` disabled for a full release.

Wait for **all six platform jobs and Publish tagged release** to succeed.
Then check `gh release view vX.Y.Z` and verify that every expected download is
present. A successful branch build only uploads Actions artifacts; it does
not publish a release.

Manual builds can set `mobile_only` to build Android and iOS without rebuilding
desktop packages. `source_ref` selects an existing source tag or commit, using
the workflow on the dispatch ref. These source-override builds do not publish
automatically; this is useful for preparing mobile assets for an existing tag.

## Packaging notes

- Desktop jobs build and embed the web frontend before the Rust application.
- The Flatpak job generates its Cargo vendor list from the committed lockfile.
- Android APKs currently use development signing. The iOS IPA is unsigned and
  must be signed for the user's device; it is not an App Store distribution.
- Windows packages are not Authenticode signed. macOS packages are ad-hoc
  signed, not notarized. The Homebrew cask uses the published DMG's SHA-256.
- Large uploads are retried individually. A failed publish job can be rerun
  without rebuilding successful platform jobs.
- Local NixOS activation is separate from publication. If requested, update
  the flake pin, build, switch, and restart Kog before claiming it is active.
