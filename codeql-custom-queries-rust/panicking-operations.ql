/**
 * @name Panicking operations in production code
 * @description The platform handles untrusted local input. Panics and
 *              `unwrap`/`expect` turn recoverable failures into process
 *              crashes, so they should stay in tests and fixtures.
 * @kind problem
 * @problem.severity warning
 * @id rumahl/rust/panicking-operation
 * @tags reliability
 */

import rust

/**
 * Holds if `f` is Rust source that ships in a production crate, excluding
 * tests, benches and fixtures.
 */
predicate isProductionRust(File f) {
  f.getExtension() = "rs" and
  exists(string path | path = f.getRelativePath() |
    path.matches("%src/%") and
    not exists(string blocked |
      blocked in ["%test%", "%fixture%", "%/benches/%", "%/examples/%"] and
      path.matches(blocked)
    )
  )
}

string panickingMacro() { result = ["panic", "todo", "unimplemented", "unreachable"] }

from AstNode node, string name
where
  isProductionRust(node.getLocation().getFile()) and
  (
    exists(string raw |
      node instanceof MacroCall and
      raw = node.(MacroCall).getPath().toAbbreviatedString() and
      name = raw.regexpCapture("([A-Za-z_][A-Za-z0-9_]*)$", 1) and
      name = panickingMacro()
    )
    or
    node instanceof MethodCall and
    name = node.(MethodCall).getTargetName() and
    name = ["unwrap", "expect"]
  )
select node, "Panicking operation `" + name + "` in production code; return a typed error instead."
