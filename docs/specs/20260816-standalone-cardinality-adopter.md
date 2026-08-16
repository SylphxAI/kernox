# Standalone cardinality adopter

## Goal

Prove that an application outside the root workspace can use Kernox's typed
multi-provider and optional dependency contracts in one real composition. The
consumer owns the notification domain; Kernox only resolves and injects the
declared capabilities.

## Contract

The standalone consumer's fan-out application composes:

- two independently implemented notifier plugins providing one shared typed
  capability;
- one application plugin requiring `OneOrMore` notifier providers through
  `InitializationContext::all`; and
- one `ZeroOrOne` metrics requirement, intentionally left unprovided and
  observed as `None` through `InitializationContext::optional`.

Provider order is the graph's stable plugin-identity order. The application
retains direct typed handles after initialization and dispatches without graph
lookups.

## Acceptance

`cargo run --locked --manifest-path fixtures/clean-consumer/Cargo.toml --
--fanout` must start and shut down cleanly, dispatch to both notifier
providers in deterministic order, observe the absent optional capability, and
reject any result that does not match the declared contract.

This is source-level external-consumer evidence only. It does not claim
registry publication, deployment, live adoption, a performance SLA, or stable
1.0 API maturity.
