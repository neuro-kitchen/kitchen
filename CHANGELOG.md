# Changelog

All notable changes to kitchen are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) once releases start.

## [Unreleased]

### Added

- README: what kitchen is and the planned crates.
- `.gitignore`: local notes, data, model weights, secrets and build output stay out of git.
- Licences: `LICENSE-MIT` and `LICENSE-APACHE` (MIT OR Apache-2.0).
- `SECURITY.md`: how to report a vulnerability.
- Dependabot: weekly cargo and GitHub Actions updates, minor and patch grouped.
- Supply-chain checks: cargo-deny (`deny.toml`) on pushes, pull requests and weekly.
- CodeQL scanning of the Rust sources and workflows, run once the repository is public.
