# Security policy

## Supported versions

Security fixes are provided for the latest released minor line. If no registry
release exists yet, only the default branch is eligible for coordinated fixes;
source-candidate status is not a package-release claim.

## Reporting a vulnerability

Use GitHub private vulnerability reporting for this repository. Do not open a
public issue containing exploit details, credentials, private data, or an
uncoordinated proof of concept.

Include the affected version or revision, environment, impact, reproduction
conditions, and any suggested containment. Maintainers will acknowledge the
report, establish a disclosure channel, and publish an advisory when users need
to act.

## Security boundary

Native Kernox plugins are trusted code compiled into the application and have
the host process's privileges. They are not sandboxed. Untrusted code requires
a future explicit process or WebAssembly capability boundary; installing a
crate as a Plugin does not create that boundary.
