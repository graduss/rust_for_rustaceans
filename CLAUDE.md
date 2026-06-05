# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A personal study repository for working through the book *Rust for Rustaceans* (Russian translation). It is not a library or application — it's a single binary crate (edition 2024, no dependencies) used as a scratchpad to run examples and exercises from the book.

Materials are in Russian:
- `book/` — chapter texts as `Глава_N_<Тема>.md` (plus PDF copies).
- `examples/` — per-chapter practice files (`Глава_N_<Тема>_практика.md`) containing worked examples ("Примеры") and exercises ("Задания") that supplement the corresponding `book/` chapter.

## Commands

- `cargo run` — build and run all currently wired-up snippets.
- `cargo check` — fast compile check.
- There are no tests; exercises are verified by running them and reading the output.

## Code structure and conventions

- `src/main.rs` declares one module per chapter (`mod c2;`, etc.) and calls the snippet functions to execute.
- Each chapter module (`src/cN.rs`) holds small `pub fn` snippets transcribed from the practice files:
  - `fX_Y()` — example Y from section X ("Пример X.Y" in the practice file).
  - `eX_Y()` — exercise Y from section X ("Задания").
- When adding a new chapter's code, create `src/cN.rs`, follow the same naming, and register/call the functions in `main.rs`.
