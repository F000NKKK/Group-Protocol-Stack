# Scripts

## release.ps1

Cuts a release: bumps all manifests, updates CHANGELOG.md and SECURITY.md,
commits, tags and pushes. The GitHub Actions workflow picks up the tag and
publishes to crates.io, NuGet, PyPI, npm and GitHub Releases.

```powershell
# Patch release (1.2.2 → 1.2.3)
pwsh ./scripts/release.ps1 -Bump patch

# Minor release (1.2.x → 1.3.0)
pwsh ./scripts/release.ps1 -Bump minor

# Major release (1.x.x → 2.0.0)
pwsh ./scripts/release.ps1 -Bump major

# RC pre-release (1.2.3 → 1.2.4-rc1, or 1.2.4-rc1 → 1.2.4-rc2)
pwsh ./scripts/release.ps1 -Bump rc

# Exact version
pwsh ./scripts/release.ps1 -Version 1.5.0

# Dry run (no push)
pwsh ./scripts/release.ps1 -Bump patch -NoPush
```

## bump-version.ps1

Updates the version string in every package manifest without committing or
tagging. Useful when you need to inspect the diff before a release.

```powershell
pwsh ./scripts/bump-version.ps1 -Version 1.3.0
```

After running, review the diff and commit manually, or just use `release.ps1`.

---

## SECURITY.md — supported versions

`release.ps1` regenerates the supported-versions table in `SECURITY.md`
automatically on every release.

**Support policy (built into the script):**
- Latest patch of the **two most recent minor series** → supported.
- Everything else → unsupported.
- An annotated tag whose message contains `deprecated` or `eol` is forced
  to unsupported even if policy would support it.

### Mark a patch as deprecated

Use this when a patch has a known issue but you are not releasing a new
version immediately:

```bash
git tag -a -f v1.2.2 -m "deprecated: superseded by v1.2.3"
git push origin v1.2.2 --force
```

Then re-run the release script (or just regenerate SECURITY.md manually):

```powershell
# Regenerate SECURITY.md without bumping the version
pwsh ./scripts/release.ps1 -Bump patch -NoPush
# Then commit and push the SECURITY.md change separately
git add SECURITY.md
git commit -m "docs(security): mark v1.2.2 as deprecated"
git push
```

### Mark an entire minor series as end-of-life

```bash
for tag in v1.1.0 v1.1.1 v1.1.2 v1.1.3 v1.1.4; do
  git tag -a -f "$tag" -m "eol: 1.1.x series end-of-life"
done
git push origin v1.1.0 v1.1.1 v1.1.2 v1.1.3 v1.1.4 --force
```
