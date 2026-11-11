# Changelog

All notable changes to kitchen are written here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) once releases start.

## [Unreleased]

## [0.0.1] - 2026-11-11

### Added

- Cargo workspace with the `kitchen-ui` crate (gpui-kit 0.9.1).
- The `kitchen-ui` package carries the MIT and Apache-2.0 licence texts.
- `kitchen-ui` design tokens: spacing on the 8 px grid, type scale (15 px base), radii by tier,
  pane gaps; shared neutrals (plain greys) and status colours for light and dark, checked for
  WCAG contrast; the accent `Palette` an app supplies (`LIME` for dsp-kitchen, `EOSIN` for the
  imaging app); `apply` hands them to gpui-kit's theme.
- `kitchen-ui` widgets: `Section`, `row`, `slider`, `MenuSelect`, `stepper`, `scrub_label`,
  `swatch`, `EmptyState`, `icon_button`.
- `kitchen-ui` parameter editor, generic over any op that serializes to JSON (`Params`).
- `kitchen-ui` icon bundling (`AppAssets`) and the check an app runs over its sources
  (`assert_icons_bundled`, aliased icon names included).
- Widget gallery example (`cargo run -p kitchen-ui --example gallery`) and a headless UI test.
- `kitchen-ui` accents: lumen's eosin rose (`Palette::EOSIN`) next to dsp-kitchen's lime; a user
  accent of any colour (`Look::accent`, an `Rgb` stored as `"#rrggbb"` in an app's preferences),
  turned into light and dark accents that keep the hue and pass WCAG contrast; the
  `AccentPicker` widget (the app's accent, suggested swatches, any colour), shown in the gallery.
- README: how an app depends on kitchen (git tag, local override, access while private).
- `kitchen-ui` is ready for crates.io: its own README, repository, keywords and category (not yet
  published).
- README: what kitchen is and the planned crates.
- `.gitignore`: local notes, data, model weights, secrets and build output stay out of git.
- Licences: `LICENSE-MIT` and `LICENSE-APACHE` (MIT OR Apache-2.0).
- `SECURITY.md`: how to report a vulnerability.
- Dependabot: weekly cargo and GitHub Actions updates, minor and patch grouped.
- Supply-chain checks: cargo-deny (`deny.toml`) on pushes, pull requests and weekly.
- CodeQL scanning of the Rust sources and workflows, run once the repository is public.
