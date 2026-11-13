# Security policy

## Reporting a vulnerability

Please report security issues privately by email to **hello@probnode.com**, with "SECURITY" in
the subject. Do not open a public issue for a vulnerability.

Include what you can: the affected version, a description, and steps that reproduce it. We
acknowledge reports within three working days and keep you informed until the issue is fixed.
We credit reporters in the release notes unless they prefer not to be named.

## Scope

In scope:

- the API key sent anywhere but the configured API (to a signed-link host, or across a
  redirect);
- a downloaded file left at its target path without its sha256 matching the listing;
- an object path from a response that writes outside the destination directory;
- a panic, unbounded allocation or hang on any response.

Vulnerabilities in the ProbNode service itself are welcome too; report them the same way.

## Supported versions

Security fixes go to the latest released version.
