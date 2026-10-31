## What

<!-- What does this change do? One or two sentences. -->

## Why

<!-- The problem it solves, or the upstream issue it follows (usecornercase/cornercase-terminal#123). -->

## How

<!-- The approach, and anything a reviewer should look at closely. Alternatives you discarded and why. -->

## Testing

<!-- New or updated tests, and what you checked by hand. -->

## Screenshots

<!-- For anything visible: before / after. Delete this section otherwise. -->

## Checklist

- [ ] `cargo fmt`, `cargo clippy --all-targets --all-features --locked -- -D warnings` and `cargo test --locked` pass
- [ ] `cargo-machete` and `npx -y jscpd@4.3.0` pass
- [ ] No workaround for something cornercase lacks: an upstream issue instead, named in the UI
- [ ] Code or assets taken from another project keep their licence: its licence file and a line under the README's License
- [ ] The area's file in `design/` is updated if this changes a design decision, `design/architecture.md` if it adds a module, `AGENTS.md` if it changes a rule
