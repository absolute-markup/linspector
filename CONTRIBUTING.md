# Contributing to Linspector

Designed and developed by **Absolute Markup**.

Thank you for helping improve Linspector. We welcome testing reports, usability feedback, reproducible bug reports and technical reviews. Start with the [testing guide](docs/testing.md) and [testing-report form](https://github.com/absolute-markup/linspector/issues/new?template=testing-report.yml).

## Testing and review

Use the current source and identify the exact commit tested. Report successful checks as well as failures. Keep observations separate from assumptions and mark skipped cases not run. The [verification record](docs/verification.md) and [milestone roadmap](docs/milestones.md) describe what has actually been checked.

Ordinary inspection should run without elevation. Privileged/helper installation, removal, service changes and recovery experiments belong in the [disposable Fedora VM procedure](docs/fedora-vm-testing.md). Do not bypass capability checks to make another distribution appear supported.

Reviewers can focus on provider identities and scopes, native parsing, source failure isolation, missing metadata, stale reviews, privilege boundaries, ownership checks, accessibility and performance. Useful reviews explain the affected behavior, reproduction steps and evidence. Avoid including personal system details or credentials in public reports.

## Code changes

For substantial changes, first open an issue describing the problem and proposed scope so implementation can be coordinated. Preserve the native Qt Quick/QML + Rust/CXX-Qt architecture. Keep provider-specific behavior behind its provider, missing values explicit and management capabilities unavailable until implemented and validated.

For a proposed patch, explain the resulting behavior, relevant tests and any limitations. The core verification command is:

```sh
cargo test --locked -p linspector-inventory
```

Use native integration tests only where their prerequisites apply; do not represent fixture results as native desktop validation. See the [developer guide](docs/development.md) for build and verification details.

## Source terms

The repository is available for inspection and testing. **No open-source license has been selected**, and publication does not replace the terms in [COPYRIGHT](COPYRIGHT). Third-party components retain their own licenses. Before submitting code, discuss contribution and licensing terms with Absolute Markup in an issue; do not assume an unstated license or assign rights on someone else's behalf.
