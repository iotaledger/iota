# Rust Conventions and Best Practices

These guidelines define IOTA Foundation rules and recommendations for Rust development.

## Requirements

The following rules should always be followed to the best of your ability.

### Panics and Safety

#### Justify Panics

All usages of `unsafe`, `.expect()`, `panic!()`, `unreachable!()`, or other functions that can explicitly panic under
defined conditions should be justified using a comment, or in some cases in the error message.

#### Expect over Unwrap

Unwrapping should be done using `.expect()` with an appropriate error message. Messages should follow
[these guidelines](https://doc.rust-lang.org/std/result/enum.Result.html#recommended-message-style).

#### Prefer Audited Implementations

Whenever possible, audited libraries should be used to perform unsafe behaviors, rather than implementing something
manually which could be incorrect.

#### Document Potential Panics

Functions which might panic should document this behavior clearly in its `rustdoc` comments within a `Panics` section
(see [here](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html#documenting-components) for an example).

### Style

#### Descriptive Variable Names

Use variable names between 1-4 words long that adequately describe the purpose of the variable. Avoid acronyms and
abbreviations except when the variable is extremely long.

It is acceptable to use short, non-descriptive variables in some situations. For example:

- Indexes in loops (`i`, `j`, `k`, `idx`, etc)
- Single-line mapping closures

### Organization

#### Absolute Imports

Use `crate`-level imports. Do not use `super` to specify relative import paths, as they tend to break more easily during
refactors than `crate` imports.

<small>NOTE: Relative import paths are allowed in test modules.</small>

#### Explicit Imports

Do not use the import-all wildcard (`use something::*;`). All dependencies should be _explicitly_ imported.

<small>NOTE: The wildcard is allowed for public re-exports.</small>

### Error Handling

#### Succinct Error Messages

Error messages should generally be short and to the point. Avoid multiple sentences and periods, instead use commas or
semicolons to divide message content. When writing error messages, think about what information would be most helpful in
a debugging scenario to quickly understand what the cause of the error was.

#### Error Message Style

Error messages should be lowercase and conform to
[Common Message Styles](https://doc.rust-lang.org/std/error/index.html#common-message-styles).

#### External Errors Can Be Handled

Errors which are expected to be handled by library users should be defined in such a way that they can be responded to
appropriately, usually by matching on enum variants or an error code. Avoid trait objects and convenience crates such as
`anyhow`.

### Versioning and Breaking Changes

#### Private Types

Types should be defined as privately as possible, so that updating them does not necessitate breaking changes.

#### Additive APIs

When changes are made to a public API, the changes should be additive if possible.

#### Deprecate APIs

Public API methods that are due to be deleted should be marked with `#[deprecated]` with a reason and version, so that
they may be safely deleted after the next breaking version release.

#### Non Exhaustive Types

Structs, enums, and variants that are expected to grow (such as error types) should be marked with `#[non_exhaustive]`
so that additive changes are non-breaking.

### Dependencies

#### Workspace Dependencies

Dependencies that are defined in the workspace manifest should follow these rules:

- `version` set to the most fine-grained version that is non-breaking\*
- `default-features = false`
- `features` should only contain features that are common across _all_ child crates

Dependencies that are defined in crates should follow these rules:

- If using a workspace dependency, set `workspace = true`; otherwise set `version` to the most fine-grained version that
  is non-breaking\*
- `features` contains additional features that are needed for the crate

<small> * SemVer specifies that breaking changes may occur on Major releases for stable crates (version >= `1.0.0`), and
Minor releases for crates below version `1.0.0`.</small>

#### Avoid New Dependencies

New dependencies should be well justified.

#### Avoid Unmaintained Dependencies

Unmaintained dependencies should never be added. Replacements should be found for existing unmaintained dependencies if
possible.

#### Audited Releases

Never release a version of crates with vulnerabilities reported by auditing. If there is an audit, update immediately
when a new version is published.

#### Vetted Critical Dependencies

Critical dependencies should be well vetted. A dependency is critical if replacing it would present a major development
effort.

Crates that are commonly used by the community can be considered well-vetted. Lesser-known crates should be thoroughly
scrutinized.

### Testing

#### Unwrap over Result

Use `.unwrap()` in tests rather than returning a result, so that the stack trace is printed in the output if the line
fails.

#### Unit Test Locality

Unit tests should be defined in a tests module as locally as possible to the tested code.

#### Test APIs

Avoid making non-public APIs public if they will only be used by tests. Instead, either define the tests locally or add
test-specific public APIs that are gated by the `#[cfg(test)]` attribute.

#### Descriptive Test Names

Test functions should be descriptive and should not be prefixed with `test_`.

## Recommendations

The following conventions are strongly recommended, but may not always apply. Breaking these conventions generally
demands an explanation.

In addition to the conventions listed here, follow the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html).

### Organization

#### Minimal Modules

Generally speaking, `mod.rs` files should contain little or no code.

<small>NOTE: In situations where a module folder should logically contain functionality, create a file with the same
name as the containing folder and re-export it’s members in <code>mod.rs</code>.

    something
        ├ mod.rs <-- pub use something::*;
        ⎩ something.rs

</small>

#### Small Files

Prefer small file sizes with only local struct implementations and strongly applicable code.

#### Preludes

When an API is highly reliant on traits to provide common functionality, they should be bundled in a prelude-style
public re-export for convenience.

### Error Handling

#### Contextual Errors

Detailed context should be provided on errors whenever possible.

When using `thiserror` to define errors, prefer `#[source]` over `#[from]` when defining wrapping errors. Errors that
are converted automatically by `thiserror` contain only the context provided by the wrapped error, which is often from a
3rd party library. This is frequently insufficient to discern the nature of the error when debugging, particularly if a
back trace is not available. Using `#[source]` forces the call site to manually map error types and allows a opportunity
to add helpful context to errors that may otherwise contain none.

### Style

#### Descriptive Generics

When generics and lifetimes are presented as part of a public API, they should be descriptively named. In particular,
lifetimes should describe what they constrain. Consider using multi-letter names for type parameters (e.g. `Doc` instead
of just `D`) as well if it helps clarity and readability, in particular when a type has multiple type parameters.

### Usability

#### Use Public APIs

When exposing a new public API, write accompanying examples and tests that use it.

### Documentation

#### High-Level Documentation

Documentation about high-level library usage should be located in the top-level `lib.rs`. This documentation should be
extensive and cover most topics that a user needs to know in order to use the API.

#### Minimal Code in Rustdocs

Code examples in `rustdoc` comments should be used sparingly in code that needs usage examples due to complexity or
non-obvious behaviors.

### Features

#### Avoid New Features

New features should be strongly justified, and other options should be considered first.

## Links

[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html)

[Rust Reference](https://doc.rust-lang.org/reference/introduction.html)

[The Cargo Book](https://doc.rust-lang.org/cargo/index.html)

[Rust Cheatsheet](https://cheats.rs/)

[Error Handling in Rust](https://nrc.github.io/error-docs/intro.html)

[Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/introduction.html)

[Nextest](https://nexte.st/)
