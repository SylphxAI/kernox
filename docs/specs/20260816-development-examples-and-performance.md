# Development examples and performance contract

## Goal

Keep Kernox in a long pre-1.0 development phase while making its useful
composition shapes executable and measuring the costs that matter to adopters.
Examples must demonstrate application-owned domain code using Kernox; they must
not turn the kernel into a business framework.

## Development policy

- No stable 1.0 publication is permitted while public APIs and lifecycle
  semantics are still expected to evolve.
- A package/version candidate is not a registry release. Tags, publication,
  and runtime adoption remain separately authorized states.
- Pre-1.0 API changes must update contracts, examples, and benchmarks in the
  same source change when their behavior is affected.

## Example coverage

The repository keeps one reference application per distinct composition shape:

1. `order-app`: a host-neutral domain graph reused by long-lived and warm
   serverless hosts, with fresh invocation scopes.
2. `checkout-app`: an application service with two compatible payment
   providers selected by an explicit binding, showing provider replacement
   without changing the domain port.
3. `worker-app`: a domain worker requiring the official Tokio task capability,
   showing named admission, cancellation, and clean drain on shutdown.

Each example owns its domain traits and plugin descriptors. Kernox supplies
graph validation, typed injection, lifecycle ownership, and host boundaries;
it does not supply payment, order, or worker policy.

## Performance contract

The goal is measured proportional overhead, not an unprovable claim of
“maximum” speed. The benchmark matrix covers:

- graph construction at sparse and dense control-plane sizes;
- indexed dependency acquisition during initialization;
- application boot and reverse shutdown;
- warm invocation-scope admission and release; and
- steady-state direct typed-handle calls versus hand-written direct calls.

The normal-call path must remain direct typed-handle dispatch after composition;
it must not perform graph lookup, serialization, locking for Kernox metadata, or
event dispatch. A regression is actionable when a representative benchmark
exceeds its recorded baseline or the declared steady-state budget, not when a
single noisy sample moves.

## Acceptance

- Every example builds and runs from a clean workspace.
- `checkout-app` proves explicit provider binding and unchanged domain code.
- `worker-app` proves supervised task cancellation and clean shutdown.
- `order-app` continues to pass the three-plugin conformance oracle.
- The benchmark report records distributions and environment, and documents
  unmeasured dimensions instead of claiming universal optimality.
