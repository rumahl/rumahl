/**
 * @name Process execution outside the runtime supervisor
 * @description rumahl confines host process execution to the platform-buildroot
 *              runtime supervisor. Any other production crate that spawns a
 *              process with `Command::new` breaks that boundary.
 * @kind problem
 * @problem.severity error
 * @id rumahl/rust/process-execution
 * @tags security
 *       external/cwe/cwe-078
 */

import rust

/**
 * Holds if `f` is application Rust source (production crates other than the
 * buildroot runtime supervisor), excluding tests, benches and fixtures.
 */
predicate isApplicationRust(File f) {
  f.getExtension() = "rs" and
  exists(string path | path = f.getRelativePath() |
    path.matches("%src/%") and
    not exists(string blocked |
      blocked in ["%test%", "%fixture%", "%/benches/%", "%/examples/%", "%platform-buildroot%"] and
      path.matches(blocked)
    )
  )
}

from CallExpr call
where
  isApplicationRust(call.getLocation().getFile()) and
  exists(PathExpr target | target = call.getFunction() |
    target.getPath().toAbbreviatedString().matches("%Command::new")
  )
select call, "Host process execution is restricted to the platform-buildroot runtime supervisor."
