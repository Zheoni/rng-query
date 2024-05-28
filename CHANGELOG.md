# rng-query Change Log

## Unreleased - ReleaseDate

- Add `t` flag to treat entries as _text_.
- Add random color generation.
- Add uuid V4 generation.

## 2.0.1 - 2023-02-27

Internal improvements and CLI args renames. Old arg names are still supported
but hidden and may be removed or repurposed in a future release.

## 2.0.0 - 2023-02-27

This release focuses on simplify things while keeping the core functionality.

### Language

- Add support for subqueries.
- Remove stack (and 'p' flag).
- Remove multiple queries separated by ';'.
- All entries are evaluated in a query now. Removed 'e' and 'E' flag.

### CLI

- Remove read from files.
- STDIN now only supports adding entries. Entries can still be expressions or
  just data.
