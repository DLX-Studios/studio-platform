# Define Designer trust and capability boundaries

Type: grilling
Status: resolved
Blocked by: 01, 02, 07, 08, 09, 10

## Question

What trust domains and capability checks must separate the first-party Designer, embedded local database, cloud sync service, live agents, MCP clients, sandboxed extensions, compiler, and generated Runtime applications?

## Answer

Separated trust domains with capability checks at every boundary: first-party Designer, embedded local database, cloud sync service, live agents, MCP clients, sandboxed extensions, compiler, and generated Runtime applications. Closed protocol between domains; least-privilege, consent-recorded capabilities. Guests, agents, MCP clients, and extensions never touch a database handle or query language; all data flows through host-mediated helpers. Compiler output is signed and verified before launch.

See [CONTEXT.md](../../../CONTEXT.md) for the settled glossary.
