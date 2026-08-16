# Verified application conformance

## Goal

Provide one reusable, executable oracle for the Kernox North Star path:
an application composes at least three source-attributed plugins, boots through
the real runtime, and shuts down without lifecycle failures.

The oracle proves composition and lifecycle conformance. It does not prove that
the plugins have independent legal owners, that packages were published, or
that an application is deployed or serving live traffic.

## Contract

`kernox-testkit::verify_application(ResolvedApp)` consumes a resolved
application and returns a `ConformanceReport` only when all of these hold:

1. the graph contains at least three plugins;
2. every plugin has source attribution with a package and repository;
3. source package names are unique within the application;
4. runtime initialization and startup complete successfully; and
5. shutdown completes with no cleanup failures.

The report contains the plugin count, source package names, and the exact
startup and teardown orders observed from the immutable graph. The function
consumes the application so the checked lifecycle is the lifecycle that was
actually exercised.

## Non-goals

- inferring ownership or release status from descriptor metadata;
- adding a runtime registry, service locator, or host-specific policy;
- replacing the existing lifecycle, host, or graph tests; and
- treating a passing conformance check as a package, deployment, or live claim.

## Acceptance and validation

- `kernox-testkit` rejects fewer than three plugins, missing source metadata,
  and duplicate source packages with stable typed errors;
- a three-plugin source-attributed probe passes and reports deterministic order;
- `examples/order-app` passes the oracle through its real `compose()` path; and
- the repository verification entrypoint and both reference host binaries pass.
