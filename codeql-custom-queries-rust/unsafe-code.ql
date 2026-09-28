/**
 * @name Unsafe Rust in production code
 * @description rumahl runs as a local system service. Every `unsafe` block or
 *              function in a production crate needs an explicit safety review,
 *              so this query lists the ones currently in the tree.
 * @kind problem
 * @problem.severity warning
 * @id rumahl/rust/unsafe-code
 * @tags security
 *       reliability
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

from AstNode node
where
  isProductionRust(node.getLocation().getFile()) and
  (
    node instanceof BlockExpr and node.(BlockExpr).isUnsafe()
    or
    node instanceof Function and node.(Function).isUnsafe()
  )
select node, "Unsafe Rust in production code requires a documented safety review."
