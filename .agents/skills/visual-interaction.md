# Visual interaction for Studio planning

Use this guide in Studio workflows that ask the project manager to choose between meaningful options or shape a visual direction. The goal is to make planning decisions easier to compare and answer in the current agent chat.

## Discover the current host's capabilities

- Before presenting the first substantial choice, inspect the tools and skills available in the current agent session for native single-select or multi-select questions, visual canvases, inline HTML or diagrams, image or mockup generation, and other user-facing presentation features.
- Use native question controls when they return the user's selected value to the agent. Keep an option for a custom answer when the host supports it.
- When a relevant presentation or design skill is available, read and follow it. Discover skills through the current skill catalog and the available skill-discovery workflow; refer to skills by name. Do not assume another agent host has the same tools, skill names, or installation paths.
- Do not install tools or skills, connect an external service, publish an artifact, or create project assets just to present a planning choice.

## Present decisions visually

- For material choices, use the best supported chat UI: structured choices first when available; otherwise use a rendered comparison, choice board, sketch, diagram, or other visual artifact supported by the current host.
- Use visual formats that match the decision:
  - Project shape: compare best-fit cases and show a small example folder tree for each option.
  - App type: use a choice board or compact matrix that separates **Browser**, **Installed desktop**, and **Installed mobile** channels, then shows implementation and supported/limited/unverified OS targets. Do not combine responsive browser access and native installation into one ambiguous “all devices” label.
  - Console scope: show the public app catalog and admin planning routes as a route map; compare database or hosting options in a decision table or cards.
  - Design direction: when useful, show palette swatches, type samples, layout sketches, or a small moodboard before asking the user to choose. Use image generation for an exploratory mockup only when it materially helps the decision and an inline image tool is available; do not save it as a project asset or replace existing assets unless the user asks.
- Keep the corresponding question concise and visible beside or immediately after the visual. Mark a recommended choice and explain why in one sentence. Accept an answer by option label and allow the user to propose another option.
- Use visuals at decision points, not for every conversational turn. Group related high-impact questions into a small set and carry answers forward between Studio skills.

## Make combinations easy to select

- Keep independent axes separate. For app planning, ask for delivery surfaces first (browser, installed desktop, installed mobile), then recommend compatible implementation profiles. Do not mix mutually exclusive architectures such as Studio Platform and a standalone SvelteKit app into one multi-select list.
- For a multi-select question, provide a native multi-select control with **Select all** when available, plus **Custom**. When the host only supports single-select choices, offer useful combination presets such as **All surfaces**, **Browser + installed mobile**, **Browser + installed desktop**, and **Installed mobile + desktop only**. Include single-surface choices where relevant.
- Accept ordinary replies such as “all,” “all of the above,” “both,” “web and mobile,” or “desktop too” as selections from the options just shown. Map them to the corresponding option set without making the user retype every item. Treat “both” as the two alternatives explicitly contrasted in the immediately preceding question; if more than two combinations remain plausible, ask one short clarification.
- Include a **Custom choice** so the user can describe another combination. Before planning proceeds, briefly restate the mapped selection (for example, “Browser + installed mobile; no desktop package”) and ask for correction only if the mapping is uncertain.
- Do not add “All of the above” to mutually exclusive choices such as repository shapes, or where combining the options is technically invalid. Explain that the choice is exclusive and offer a separate multi-select question for the dimensions that can be combined.

## Be clear about interaction and accessibility

- Confirm whether a visual is actually interactive. If it is only a rendered diagram or mockup, ask the user to answer by its label; do not imply that clicks or edits will be sent back to the agent.
- Keep a concise text equivalent for the choices and any important visual information. Make labels readable, use clear contrast, and avoid relying on color alone.
- If the host exposes no suitable visual or structured-input capability, use a compact Markdown table, Mermaid diagram, or folder tree and ask directly in text. Do not claim that a UI control or artifact was shown when it was not.
- When a UI control cannot preserve a needed nuance, use it for the bounded selection and invite the user to add context in text.
