# Distribution runbook

How to package, sign, publish and support a Pigeon release on macOS, Windows and Linux.

## Current state

Check this table before the first public release, and update it as items are done.

| Item | State |
| --- | --- |
| macOS app and disk image | Built on Apple Silicon, and the app was started with `--screenshot`. A normal first launch hasn't been tried, including carrying over old settings in the running app. Signed for the local computer only, not notarized. |
| Windows installer and zip | Built, and contents inspected. **Never started on a Windows computer.** Unsigned. |
| Linux `.deb` and `.tar.gz` | Built, and contents inspected. **Never started on a Linux desktop.** Core tests pass on Linux. |
| GitHub workflows | Written. **Never run**, because the project isn't on GitHub yet. |
| Automatic updates | Not available. People download each new version. |

### One-time decisions before going public

- [ ] **Licence.** None has been chosen, so add a `LICENSE` file and tell users the terms.
- [ ] **Third-party notices.** Pigeon includes open-source libraries and fonts whose licences require attribution.
      Generate a notices file, for example with `cargo about`, and ship it with each package.
- [ ] **Name.** Check that "Pigeon" and the icon are free to use for this kind of product.
- [ ] **Publisher details.** Replace the placeholders: `app.pigeon.desktop` in `packaging/macos/Info.plist`,
      `Maintainer` in `packaging/docker/package.sh`, and `Publisher` in `packaging/windows/pigeon.nsi`.
- [ ] **Security contact.** Fill it in in `SECURITY.md`.
- [ ] **Signing certificates.** See each system below.

## Release steps

1. **Update the version** in `Cargo.toml` under `[workspace.package]`. Use `MAJOR.MINOR.PATCH`.
2. **Refresh the lock file** with `cargo build`, and update `CHANGELOG.md`.
3. **Check quality:** `cargo fmt --all --check`, the clippy command from the [build guide](build.md), and
   `cargo test --workspace`, then `cargo test --workspace -- --ignored`.
4. **Merge** to `main` and wait for CI to pass on all three systems.
5. **Tag it:** `git tag v1.2.3 && git push origin v1.2.3`. The tag must match the version or the build stops.
6. **Wait for the Release workflow.** It builds every package and creates a **draft** release with `SHA256SUMS.txt`,
   using the version's `CHANGELOG.md` section as its notes, so the heading must be exactly `## X.Y.Z`.
7. **Verify** the packages using the checks below, on clean computers.
8. **Publish** the draft on GitHub. Nothing is public until you do this.

To build by hand instead, follow [Building packages](build.md#building-packages), then add checksums:

```sh
cd dist && shasum -a 256 *.dmg *.exe *.zip *.tar.gz *.deb > SHA256SUMS.txt
```

## macOS

**Output:** `Pigeon-<version>-macos-<arch>.dmg`, containing `Pigeon.app`.

The script builds for the Mac it runs on, so GitHub's Mac produces an Apple Silicon build. Intel Macs need a build
on an Intel Mac, or a new step using the `x86_64-apple-darwin` target and `lipo`.

Without signing, people see a warning and must use **Open Anyway**. To remove it:

1. Join the Apple Developer Program, and create a **Developer ID Application** certificate.
2. Save a notarization login once:
   `xcrun notarytool store-credentials pigeon --apple-id you@example.com --team-id TEAMID`
3. Build:

```sh
PIGEON_MACOS_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
PIGEON_NOTARY_PROFILE=pigeon ./packaging/macos/package.sh
```

The script signs the app, builds the disk image, then signs, notarizes and staples it. This path hasn't been
exercised yet, because no certificate was available.

**Verify:**

```sh
codesign --verify --deep --strict dist/macos/Pigeon.app
spctl --assess --type open --context context:primary-signature dist/Pigeon-*.dmg   # once notarized
```

Then open the disk image on another Mac, drag Pigeon to Applications and open it.

## Windows

**Output:**

- `Pigeon-<version>-windows-x86_64-setup.exe`: installs for the current user without administrator rights, adds a
  Start menu entry and an uninstaller.
- `Pigeon-<version>-windows-x86_64.zip`: a portable copy.

Both need Windows 10 or newer, and Team Sync needs [Git for Windows](https://git-scm.com/download/win).

Without signing, SmartScreen shows "Windows protected your PC". To remove it:

1. Buy a code signing certificate, or use a cloud signing service.
2. Sign **`pigeon.exe` first**, then the installer, so both carry a signature. This needs a signing step added
   between the build and `makensis` in `packaging/docker/package.sh`. For example, with `osslsigncode`:

```sh
osslsigncode sign -pkcs12 cert.pfx -pass "$PASSWORD" -t http://timestamp.digicert.com \
  -n Pigeon -in pigeon.exe -out pigeon-signed.exe
```

SmartScreen trust builds up over time, so a new certificate may still warn at first.

**Verify** on a clean Windows computer:

- [ ] The installer runs without asking for administrator rights.
- [ ] Pigeon appears in the Start menu with its icon, and opens with no console window behind it.
- [ ] A request sends, and Team Sync can pull and push with no black windows flashing.
- [ ] Uninstalling from Settings removes the program and keeps the workspace.

## Linux

**Output:**

- `pigeon_<version>_amd64.deb`: for Debian and Ubuntu, and installs the `git` it depends on.
- `Pigeon-<version>-linux-x86_64.tar.gz`: for other distributions. Unpack it and run `./install.sh`, which installs
  for the current user.

Both need glibc 2.35 or newer, which means Ubuntu 22.04, Debian 12 or newer, and a working graphics driver.

**Install:** `sudo apt install ./pigeon_<version>_amd64.deb`. **Remove:** `sudo apt remove pigeon`.

Publishing through `SHA256SUMS.txt` is enough to begin with. For automatic updates through `apt`, host a signed
repository.

**Verify** on clean Ubuntu 22.04 and on a current release, under both X11 and Wayland:

- [ ] Pigeon appears in the applications menu with its icon, and opens.
- [ ] A request sends, and Team Sync can pull and push.
- [ ] **Import › Choose files** opens the file picker.
- [ ] `sudo apt remove pigeon` removes it cleanly.

## Repository secrets

Set these under the repository's **Settings › Secrets and variables › Actions**. The workflow runs without them and
produces unsigned packages.

| Secret | Value |
| --- | --- |
| `MACOS_CERTIFICATE` | The Developer ID `.p12` file, base64 encoded |
| `MACOS_CERTIFICATE_PASSWORD` | Its password |
| `MACOS_SIGN_IDENTITY` | The certificate's full name |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | The notarization login, with an app-specific password |

Keep certificates out of the repository, and renew them before they expire.

## If a release goes wrong

- **Before publishing:** delete the draft and the tag, fix it, and tag again with the same version.
- **After publishing:** don't replace files in a published release, because people's checksums would stop matching.
  Mark it as a pre-release, or delete it with a note saying why, and publish a new patch version.
- **Older version:** people can reinstall the previous package. Workspaces and settings stay in place.
- **A format change:** a workspace is shared, so note it prominently in the changelog if teammates must upgrade
  together. Newer files are read by older versions only as far as the fields they know.

## After a release

- Watch for issues, and ask for `crash.log` (see [Troubleshooting](troubleshooting.md#reporting-a-problem)).
- Pigeon sends no usage data, so reports from people are the only signal.
- Update the [Current state](#current-state) table.
