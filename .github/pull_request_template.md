name: Pull Request

about: Submit a code change
title: ''
labels: []
---

## Summary

<!-- One or two sentences describing what this PR does. -->

## Related issue

<!-- Link the issue this PR fixes, e.g. "Fixes #42". Delete this section if not applicable. -->

## Type of change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (existing functionality stops working)
- [ ] Documentation update
- [ ] Refactor (no functional change)

## How to test

<!-- Step-by-step instructions for a reviewer to verify the change. -->

## Checklist

- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all -- --check` is clean
- [ ] Documentation updated (if applicable)
- [ ] Added or updated tests (if applicable)
