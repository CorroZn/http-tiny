# AGENTS.md

## Introduction

This project is called "http-tiny". It is a lightweight HTTP server written in Rust. It serves static websites. The document root, binding IP address, and port are user customizable by running the executable with the appropriate arguments.

The goal of this project is to provide an HTTP server that is easy to deploy, lightweight, and fast. Deployment target systems can range from embedded systems or other systems with very low hardware specifications to container images and other virtual environments with no runtime libraries and core utilities present.

This HTTP server will run entirely in user-space and can also be executed by unprivileged users if they choose a custom document root that they have access to and an unprivileged listening port.

## Purpose

This repository welcomes both human-written and AI-generated code, provided contributions receive appropriate review and meet this project's quality standards.

There are no requirements regarding how code is written or which IDEs, AI models, coding assistants, or other development tools are used during its creation.

There are no minimum requirements for human authorship, provided the standards described in this AGENTS.md for correctness, stability, performance, security, maintainability, and portability are met.

The primary goal is to produce code that is simple, correct, secure, portable, and easy for humans to understand, review, and maintain.

Contributions are evaluated solely based on their technical quality and compliance with this project's standards, regardless of the tools or methods used to produce them.


## General Principles

All code must always satisfy the three core requirements of this project: fast, simple, and secure. If a solution sacrifices one or more requirements, justify the tradeoff explicitly and why the project still benefits from the change.

- Favor predictable behavior over feature richness.
- Prefer small, focused changes.
- Do not modify unrelated code.
- Do not add features, configuration options, or behavior unless requested.
- Do not redesign existing functionality unless required by the task.
- Follow the existing architecture and coding style.
- Favor readability and maintainability over cleverness.
- Avoid unnecessary dependencies.
- Preserve backward compatibility unless the task explicitly requires otherwise.
- Do not make assumptions. If requirements are ambiguous, ask for clarification.

## Priority Order

When instructions conflict, follow this order:

1. Preserve correctness and security.
2. Preserve existing public behavior and compatibility.
3. Follow explicit task requirements.
4. Follow existing project conventions.
5. Prefer the simplest maintainable implementation.
6. Optimize only when there is evidence that optimization is needed.

## Before Making Changes

Before modifying code:

- Inspect the relevant modules and existing patterns.
- Understand ownership, lifetimes, error handling, and module boundaries.
- Look for existing helpers or abstractions before adding new ones.
- Avoid redesigning code unless the task requires it.
- Use the existing implementation as the source of truth instead of making assumptions.

## Rust

- Prefer safe and stable Rust.
- Always prefer existing safe abstractions over creating new unsafe code.
- Document the safety reasoning for every `unsafe` block and explain why a safe alternative was not used.
- Follow idiomatic Rust practices.
- Format code with `cargo fmt`.
- Keep Clippy warnings to a minimum.
- Ensure the project compiles before proposing changes.
- Add or update tests when changing behavior.
- Treat every function signature as a contract, not just a type declaration.
- Request the minimum ownership, mutability, and access required.
- Every parameter is a permission request—ask for no more than the implementation actually needs.
- Make signatures reflect exactly what the function reads or modifies.
- Prefer borrowing specific values or fields over borrowing an entire object (e.g. `&mut self`) when possible.
- When the borrow checker complains, first consider whether the signature is too broad before introducing clones or interior mutability.
- Keep implementation details behind stable function signatures; changing the body should not require changing the contract.
- Optimize APIs for clear ownership and borrowing semantics, not just convenience.

## Code Quality

- Write code that another developer can quickly understand.
- Avoid unnecessary abstractions, indirection, and over-engineering.
- Keep functions and modules focused on a single responsibility.
- Add comments when they improve understanding of the code.
- Comments should explain intent, design decisions, assumptions, safety considerations, or non-obvious behavior.
- Avoid comments that simply repeat what the code already makes clear.
- Keep comments concise, accurate, and easy for another developer to understand.
- Document public functions, complex logic, and tricky edge cases when appropriate.
- Leave the codebase in a better state than you found it.

Good comments explain:

- Why a workaround exists.
- Why an unsafe operation is sound.
- Why a seemingly unusual implementation was chosen.

Avoid comments that only explain:

- What the next line of code does.
- Obvious control flow.
- Variable names or basic operations.

## Project Structure

- Organize code by feature or responsibility.
- Place new features in their own `.rs` module files whenever practical.
- Avoid growing large source files with unrelated functionality.
- Keep modules focused and cohesive.
- Prefer clear module boundaries over deeply nested abstractions.
- If a feature becomes large, split it into a dedicated module directory with a `mod.rs` or equivalent module layout.
- Keep public interfaces small and well-defined between modules.

## Portability

This project is intended to be portable across operating systems, C libraries, and CPU architectures.

When making changes:

- Prefer portable Rust over platform-specific implementations.
- Avoid OS-specific APIs unless they are already part of the project or explicitly required.
- Avoid architecture-specific code unless absolutely necessary.
- Prefer compile-time feature detection over runtime assumptions.
- Minimize the use of conditional compilation (`#[cfg(...)]`).
- When conditional compilation is necessary, isolate platform-specific code behind small interfaces.
- Do not introduce assumptions about pointer size, endianness, or CPU features.
- Favor solutions that work across as many Rust targets as practical.

## Dependencies

- Avoid adding dependencies unless they provide significant value in performance, safety, correctness, or maintainability.
- Before adding a dependency, check whether the standard library or existing dependencies already provide the required functionality.
- Prefer mature, actively maintained crates.
- Consider licensing, security, portability, and static linking impact before adding dependencies.
- Prefer small, focused crates over large frameworks.
- Avoid dependencies that require system libraries unless there is a strong reason.

## Static Linking

The project is intended to support statically linked binaries using musl.

When making changes:

- Avoid introducing dependencies that complicate or prevent static linking.
- Prefer pure Rust dependencies over native libraries when practical.
- Avoid unnecessary use of the Foreign Function Interface (FFI).
- If a native dependency is required, clearly justify why it cannot be avoided.

## Cross Compilation

- Do not assume the host architecture is the only supported target.
- Avoid architecture-specific assumptions.
- Prefer solutions that compile across common Rust targets.
- Consider musl-based static builds when evaluating portability.

## Performance

- Prefer simple correct implementations before optimizing.
- Avoid unnecessary allocations in request handling paths.
- Avoid blocking operations in hot paths unless intentional.
- Do not introduce complexity for speculative performance improvements.
- Benchmark before making performance-driven changes.

## Security

All changes should be reviewed and validated before they are considered ready for inclusion.

- Validate inputs.
- Avoid introducing unnecessary attack surface or security-sensitive complexity.
- Do not weaken existing security guarantees.
- Never introduce secrets, credentials, API keys, personally identifiable information, or other forms of private data.
- Highlight any security-relevant changes in the pull request description.

## HTTP Server Security

When modifying request handling:

- Prevent directory traversal vulnerabilities.
- Resolve requested paths relative to the configured document root.
- Ensure path canonicalization cannot escape the document root.
- Avoid following unexpected filesystem paths or symlinks unless explicitly intended.
- Handle malformed HTTP requests safely.
- Avoid resource exhaustion vulnerabilities from unbounded input sizes.
- Do not leak internal filesystem details in responses.

## Error Handling

- Prefer returning meaningful errors over panicking.
- Avoid `unwrap()` and `expect()` in runtime paths.
- If `unwrap()` or `expect()` is used, document the invariant that makes failure impossible.
- Preserve error context when propagating failures.
- Avoid silently ignoring errors.
- Prefer typed errors when they improve clarity.

## Licensing

- Do not copy code from third-party sources unless the contributor can legally submit it under this project's GPL-2.0-only license.
- When in doubt, implement functionality from first principles instead of copying existing implementations.
- Preserve existing license and copyright notices.

## Testing

Before considering a change complete:

- Run formatting checks.
- Run relevant tests.
- Run the full validation suite when practical.
- Report which validation commands were run and whether they passed.
- If a validation step cannot be run, explain why.

Standard validation:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test