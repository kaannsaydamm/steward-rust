# Security Policy

Steward executes tools, manages model-provider credentials, starts MCP processes, and persists local workflow state. Security reports are taken seriously, especially when they involve command execution, credential exposure, sandbox escapes, authorization bypasses, unsafe defaults, or access outside the configured workspace.

## Supported versions

Security fixes are applied to the latest released version and the current `main` branch. Older versions may not receive patches.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability.

Use GitHub's private **Report a vulnerability** function in the repository's Security tab. Include, where possible:

- the affected version or commit;
- the operating system and installation method;
- a minimal reproduction;
- expected and actual behavior;
- the security impact;
- relevant logs with credentials and personal data removed;
- any suggested mitigation.

If private vulnerability reporting is unavailable, open a public issue containing no vulnerability details and ask the maintainer to establish a private contact channel.

## Disclosure and remediation

The maintainer will assess the report, reproduce it when possible, determine affected versions, and coordinate a fix and disclosure. Please allow time for remediation before publishing technical details.

## Scope and safe testing

Only test systems, accounts, workspaces, and model-provider credentials that you own or are explicitly authorized to use. Do not access third-party data, disrupt services, retain secrets, or perform destructive testing.

## Security-related configuration

- Keep provider credentials outside source control.
- Bind local services only to trusted interfaces.
- Review tool approval policies before enabling write or command-execution capabilities.
- Treat third-party MCP servers and skill bundles as executable code.
- Back up `~/.steward` before testing migration or destructive maintenance behavior.
