# Project structure

This project contains a reference implementation for IEEE 1815.2 (https://standards.ieee.org/ieee/1815.2/7731/), both for the control station and the outstation. It will be used to test control stations and outstations for compliance with the standard.

Glossary:
- AI/AO: analog input/analog output
- BI/BO: binary input/binary output
- These indices are set by IEEE 1815.2 and cannot be changed.

Possible configurations:
- reference control station <-> reference outstation (for development and testing)
- reference control station <-> 3rd party outstation (for compliance testing)
- 3rd party control station <-> reference outstation (for compliance testing)

- monorepo
- backend: Rust code for IEEE 1815.2 Test Tool
  - src/outstation: outstation implementation. The entry point is src/outstation/main.rs.
  - src/control_station: control station implementation
- frontend: React/TypeScript code for mesa tool UI

# Rust
- do not modify registry.rs; instead, modify generate_registry.py and run it to regenerate the file
- prefer ? over unwrap() or expect()
- if using unwrap, prefer expect with a descriptive message
- avoid single-letter variable names except for loop indices; prefer descriptive names
- this isn't used yet, so breaking changes are allowed
- prefer Arc::clone(&self.arc_field) over self.arc_field.clone()
- Avoid `as`; prefer `.into` to avoid truncation.

# General
- keep changes small and focused on a single goal; do not recommend new functions or modules unless necessary
- prefer semantically clear, simple and easy to read code over time and space optimized code. Avoid tricks that reduce readability or developer experience.
- Do not use single-character variable names.
- Do not swallow exceptions or return early in error cases. Prefer explicit error handling and clear error messages.
- Prefer replace_string_in_file over cat for editing files.
- Offer ideas for improving the codebase, but do not make those improvements without asking or explicit instructions to do so.
- Avoid using grep/ls/cat in the command line when accomplishing your tasks; prefer built in VS code tools.
- Prioritize correctness and maintainability over performance.
- Backwards compatibility is not important right now. We control the entire stack, and it is unreleased. When refactoring, focus on long-term goals.
- This codebase is pre-alpha; do not assume any patterns or structures are set in stone.
- Avoid using find -exec or xargs in the command line; prefer built in VS code tools for searching and editing across files.
- Don't search through git history unless explicitly asked to. Focus on the current state of the codebase.
- Use simple language; avoid jargon.
- Assume the back end and front end are already running.

# Front end
- all npm commands for the front end should be run from ./frontend
- CSS should go in CSS module files alongside the relevant component
- When adding a dependency, npm install it rather than adding it manually to package.json
- Never use fetch directly; instead, use generated API client functions in ./frontend/src/api

# profile.json
- This is a large file. To see examples:
  - BI: around line 800
  - BO: around line 200
  - AI: around line 21100
  - AO: around line 4700

# Standard reference

If the file exists, you can use ./ignore/ieee-1815-2.md to find relevant sections of the standard.
