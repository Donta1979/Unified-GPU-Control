# Unified GPU Control v1.8 — isolated build-only checkpoint

This archive is based on v1.7 and contains only the manual `unified-gpu-build-check.yml` GitHub Actions workflow. The other upstream workflows have been excluded to prevent automatic package builds or publishing on push.

The workflow runs `cargo check --locked -p lact-gui` on a disposable GitHub Ubuntu runner. It does not install anything on the user's PC, and it does not generate an executable.

**No successful compile has been verified.** Do not install this source package.

To run, put the contents of this folder at the root of a new GitHub repository, enable Actions, then open Actions > Unified GPU Control - Manual Build Check > Run workflow.

Note: GitHub may still run other repository-level features configured independently. Use a new empty repository, not a fork of LACT.
