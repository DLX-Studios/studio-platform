# Scaffold mechanics

## Destination rules

The CLI accepts a slug of at most 64 ASCII bytes: lowercase letters, digits,
and interior hyphens, beginning and ending with a letter or digit. Its
destination is `<working-directory>/<slug>`; `studio new .` and a path such as
`studio new apps/my-app` are not valid substitutes for selecting a parent.

For a new folder:

```sh
# Run from the intended parent, using the tool's working-directory setting.
studio new my-app --template blank
studio check my-app/app.studio
studio dev my-app
```

The current gallery has `blank`, `ecommerce`, and `social`; use CLI discovery
because the gallery can evolve. Local templates currently come from the
platform checkout associated with the binary. A missing gallery is a CLI
installation problem; diagnose that path rather than substituting a web app.

For an existing empty folder, run the same command from its parent using its
basename. If it already contains agent config, `.git`, or other files,
`studio new` treats it as occupied. Preserve those files: scaffold the chosen
slug in a temporary parent, inspect the result, and transfer only intended
app files into the user folder. Check destination collisions first; existing
app sources call for reconciliation or extension. Do not use `--force` to
silently clear a user's working folder. If its basename is not a valid slug,
use a valid agreed app slug in staging while retaining the chosen destination.

When the MCP server is already bound to the destination, `project.write_file`
with `expected_revision: null` can publish each new text file as create-only.
It cannot make parent directories; create only the directories the app needs
through normal filesystem tools. A server bound to the parent must be
reconnected in the app workspace before using its source/preview tools.

## What gets generated and checked

The built-in blank template produces:

```text
my-app/
  manifest.json
  app.studio
```

Gallery metadata `template.json` is omitted. The CLI rewrites the manifest
ID to `com.studio.<slug-with-dots>`, display name to title case, and version
to `0.1.0`. It keeps the other manifest fields, including publisher metadata;
that metadata does not provision a trusted release publisher.

The CLI stages the template, parses/validates/lowers `app.studio` with the real
Studio Script compiler, then renames the staged tree into place. It does not
create `APP.md` / `AGENTS.md`, invoke ASC, or package a release bundle during
this self-check. Avoid repeating an unchanged source check merely to count it
as a second validation; run a check after subsequent edits.

## Toolchain and release boundary

Scaffolding and checking modern Studio Script do not require a browser or a
JavaScript runtime. Native preview needs an available display/backend.
Wasm compilation and release packaging use the platform's AssemblyScript
toolchain and its configured launcher. Inspect actual diagnostics if compiler
dependencies are missing; the installed binaries do not bundle every compiler.

`studio build my-app` writes the CLI's release archive under the app's `build/`
directory. Development keeps editable `.studio` text files loose in the app
folder. A release `.studio` archive is a distinct binary artifact despite the
shared extension. It is never the source folder to edit for hot reload.

For release launch, use `studio-app --bundle <absolute-bundle-path>` or MCP
`runtime.start` with a project-relative bundle path. Publisher trust must be
configured through the existing runtime trust mechanism. CLI development
signing is not production publisher provisioning.

In the platform checkout, verify behavior against `crates/studio-cli/src/new.rs`,
`templates/`, and `crates/studio-cli/src/build/mod.rs`; use current executable
help for the installed version. Some older planning docs describe a broader
self-check than the code currently performs.
