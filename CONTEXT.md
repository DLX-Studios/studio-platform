# Studio Platform Context

Studio Platform provides the native runtime and first-party tools for authoring and running Studio applications.

## Language

**Studio Designer**:
The first-party desktop application for visually creating and prototyping applications that run on Studio Runtime.
_Avoid_: Studio Canvas, Instatic port, designer plugin

**Studio Design**:
The typed, editable source of truth for a project authored in Studio Designer.
_Avoid_: HTML document, imported page, runtime tree

**Studio Runtime Application**:
An application produced from a Studio Design and executed by Studio Runtime.
_Avoid_: export target, website, generated mockup

**Agent-led Ingestion**:
The process by which an agent interprets an external file or reference and proposes equivalent Studio Design content without a dedicated format importer.
_Avoid_: format importer, automatic conversion

**Studio Library**:
The project-owned collection of assets, typed content, and bindings available to designers, agents, and the Studio Runtime Application produced from the project.
_Avoid_: CMS, media folder, fixture dump

**Asset**:
An identity-stable image, icon, video, font, or other media item in a Studio Library together with its metadata and provenance.
_Avoid_: upload, file path, blob

**Content Collection**:
A named, typed set of records in a Studio Library that can supply repeatable application content.
_Avoid_: database table, fixture file

**Content Binding**:
A typed reference from a Studio Design property to an asset or content value in its Studio Library.
_Avoid_: template string, hard-coded path

**Live Agent Edit**:
An agent-authored change applied to the active Studio Design through the same validated history and undo authority as a user-authored change.
_Avoid_: proposal, untracked mutation

**Agent Conversation**:
The persistent user-and-agent thread associated with a Studio Project, including its selected model, imported context, progress, results, and Agent References.
_Avoid_: terminal session, unrestricted agent console

**Agent Reference**:
A typed, identity-stable link in an Agent Conversation to a Studio entity or admitted source, rendered as an accessible interactive chip that opens the referenced context.
_Avoid_: decorative badge, raw file path, untyped Markdown link

**Primitive Node**:
A visual Studio Design node whose kind is directly admitted by the approved Studio Runtime component catalog and protocol.
_Avoid_: semantic design element, custom renderer node

**Reusable Composition**:
A project-owned component definition composed from approved Primitive Nodes and instantiated by reference in a Studio Design.
_Avoid_: custom runtime primitive, arbitrary component

**Runtime Projection**:
The validated transformation of Studio Design source into a Runtime-compatible `UiNode` tree, events, actions, and packaged assets.
_Avoid_: export format, renderer-owned source

**Box**:
The generic Studio visual container primitive for layout, sizing, decoration, clipping, accessibility, and child composition.
_Avoid_: div, fallback node, unknown component

**Runtime Data Store**:
The single logical embedded SurrealDB datastore owned exclusively by Studio Runtime for durable application data.
_Avoid_: shared database login, application database file

**Application Data Namespace**:
The Runtime-managed SurrealDB namespace and database that isolate one verified application's data inside the Runtime Data Store.
_Avoid_: tenant login, guest database connection
