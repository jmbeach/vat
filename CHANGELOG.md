# Changelog

## [0.1.1] - 2026-06-21

- The 3-character project-ID prefix now accepts any ASCII alphanumeric value
  (e.g. `lib`, `ui0`, `k8s`). It was previously held to the same Crockford
  base32 rules as the random suffix, which needlessly rejected natural choices.
  The auto-generated ID suffix stays Crockford base32.
