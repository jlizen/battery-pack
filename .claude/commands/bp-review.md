---
description: Review for idiomatic Rust usage 
---

Please review the code with the eye of an expert Rustacean. 

Look for places where similar logic is repeated and could be removed.

Look for poor use of Rust idioms:

* Avoid async mutexes and prefer an actor pattern.
* Avoid mutexes or cells and prefer to separate mutable state into a separate variable or field that can be passed with `&mut`.
* Avoid `panic` and `unwrap` and prefer to propagate results.

Code with an eye towards future robustness:

* Prefer exhaustive matches to `_` patterns unless you are trying to semantically express that you wish to ignore all future variants as well (i.e., future variants are highly unlikely to matter).
* When processing all fields of a struct in turn (e.g., in code that unpacks the fields of a struct and processes all of them), prefer to use `let StructName { a, b, c} = the_struct` so as to detect cases where new fields are added.

Run the `tracey` command-line tool to spec test coverage.

Also deploy subagents to validate spec comments:

* For each `[impl ...]` and `[verify ...]` spec comment in the code/tests, find the corresponding spec section and check that the code faithfully implements the spec semantics.
* For each new spec rule being added, check that the spec rule is "additive" (each spec rule should stand on their own, one spec rule should not modify or subtract from cases covered by another):
  * Good:
    * Identifiers begin with an alphabetic character and then 0 or more alphanumeric characters.
  * Bad:
    * Identifiers consist of alphanmeric characters.
    * Identifiers cannot begin with a number.

When writing tests:

* Prefer `expect_test` to snapshot an entire struct/vector versus writing tests that assert the values of individual fields/elements.
