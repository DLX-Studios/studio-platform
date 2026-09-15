// @bun
import {
  ArrowLeft,
  ArrowUp,
  Brain,
  Camera,
  ChartLine,
  ChatCircleDots,
  Check,
  Copy,
  Files,
  GearSix,
  List,
  Play,
  Plus,
  Scroll,
  Sparkle,
  Square,
  TerminalWindow,
  WarningCircle,
  Wrench
} from "./index-s2jwydk2.js";
import"./index-6mtzd7p1.js";
import {
  page
} from "./index-t5a9agca.js";
import"./index-cmbc6t50.js";
import {
  attr,
  attr_class,
  attr_style,
  clsx$1,
  derived,
  ensure_array_like,
  escape_html,
  stringify1 as stringify
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/m/_id_/_page.svelte.js
function AgentChat($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { machineId, machineName, enabled = true } = $$props;
    let messages = [];
    let models = [];
    let thinkingLevels = ["off"];
    let selectedModel = "";
    let thinkingLevel = "off";
    let draft = "";
    let streaming = false;
    let connected = false;
    let sending = false;
    let errorMessage = "";
    let copiedId = "";
    const currentModelLabel = derived(() => {
      return models.find((model) => `${model.provider}/${model.id}` === selectedModel)?.name || selectedModel || "Choose model";
    });
    const suggestions = [
      "Inspect the Rust workspace and summarize its architecture",
      "Run the test suite and fix the first failure",
      "Find the slowest part of the current build"
    ];
    function parseBlocks(text) {
      const blocks = [];
      const expression = /```([^\n]*)\n([\s\S]*?)```/g;
      let cursor = 0;
      for (const match of text.matchAll(expression)) {
        const index = match.index ?? 0;
        if (index > cursor)
          blocks.push({
            kind: "text",
            content: text.slice(cursor, index)
          });
        blocks.push({
          kind: "code",
          language: match[1].trim(),
          content: match[2].replace(/\n$/, "")
        });
        cursor = index + match[0].length;
      }
      if (cursor < text.length)
        blocks.push({
          kind: "text",
          content: text.slice(cursor)
        });
      return blocks.length ? blocks : [{
        kind: "text",
        content: text
      }];
    }
    async function sendCommand(command) {
      errorMessage = "";
      try {
        const response = await fetch(`/api/machines/${machineId}/agent`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(command)
        });
        const body = await response.json().catch(() => ({}));
        if (!response.ok)
          throw new Error(body.error ?? `Request failed (${response.status}).`);
        return true;
      } catch (error) {
        errorMessage = error.message;
        sending = false;
        return false;
      }
    }
    async function chooseModel(value) {
      const separator = value.indexOf("/");
      if (separator < 1)
        return;
      const previous = selectedModel;
      selectedModel = value;
      if (!await sendCommand({
        type: "set_model",
        provider: value.slice(0, separator),
        modelId: value.slice(separator + 1)
      }))
        selectedModel = previous;
      else
        await sendCommand({ type: "get_available_thinking_levels" });
    }
    async function chooseThinking(value) {
      const previous = thinkingLevel;
      thinkingLevel = value;
      if (!await sendCommand({
        type: "set_thinking_level",
        level: value
      }))
        thinkingLevel = previous;
    }
    $$renderer2.push(`<section class="agent-chat svelte-u0c117"${attr("aria-label", `Pi agent chat for ${machineName}`)}><header class="agent-header svelte-u0c117"><div class="agent-identity svelte-u0c117"><span class="agent-avatar svelte-u0c117">`);
    Sparkle($$renderer2, {
      size: 16,
      weight: "fill"
    });
    $$renderer2.push(`<!----></span> <div class="svelte-u0c117"><strong class="svelte-u0c117">Pi Agent</strong> <span class="connection-line svelte-u0c117"><i${attr_class("svelte-u0c117", undefined, { online: connected })}></i> `);
    $$renderer2.push(`<!--[-1-->Offline`);
    $$renderer2.push(`<!--]--></span></div></div> <button class="new-chat svelte-u0c117" title="Start a new conversation"${attr("disabled", !enabled, true)}>`);
    Plus($$renderer2, { size: 15 });
    $$renderer2.push(`<!----> New</button></header> <div class="conversation svelte-u0c117">`);
    if (messages.length === 0) {
      $$renderer2.push(`<!--[0--><div class="empty-chat svelte-u0c117"><span class="empty-icon svelte-u0c117">`);
      ChatCircleDots($$renderer2, {
        size: 27,
        weight: "duotone"
      });
      $$renderer2.push(`<!----></span> <h2 class="svelte-u0c117">Build with Pi</h2> <p class="svelte-u0c117">Ask the agent to inspect, edit, compile, or test the workspace on this machine.</p> <div class="suggestions svelte-u0c117"><!--[-->`);
      const each_array = ensure_array_like(suggestions);
      for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
        let suggestion = each_array[$$index];
        $$renderer2.push(`<button${attr("disabled", !enabled || sending, true)} class="svelte-u0c117">${escape_html(suggestion)}</button>`);
      }
      $$renderer2.push(`<!--]--></div></div>`);
    } else {
      $$renderer2.push(`<!--[-1--><div class="message-list svelte-u0c117" aria-live="polite"><!--[-->`);
      const each_array_1 = ensure_array_like(messages);
      for (let $$index_3 = 0, $$length = each_array_1.length;$$index_3 < $$length; $$index_3++) {
        let message = each_array_1[$$index_3];
        $$renderer2.push(`<article${attr_class("chat-message svelte-u0c117", undefined, { "from-user": message.role === "user" })}>`);
        if (message.role === "assistant") {
          $$renderer2.push(`<!--[0--><span class="message-avatar svelte-u0c117">`);
          Sparkle($$renderer2, {
            size: 14,
            weight: "fill"
          });
          $$renderer2.push(`<!----></span>`);
        } else
          $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<!--]--> <div class="message-stack svelte-u0c117">`);
        if (message.role === "assistant" && message.thinking) {
          $$renderer2.push(`<!--[0--><details class="thinking-block svelte-u0c117"><summary class="svelte-u0c117">`);
          Brain($$renderer2, { size: 14 });
          $$renderer2.push(`<!----> Reasoning</summary> <p class="svelte-u0c117">${escape_html(message.thinking)}</p></details>`);
        } else
          $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<!--]--> `);
        if (message.text) {
          $$renderer2.push(`<!--[0--><div class="message-content svelte-u0c117"><!--[-->`);
          const each_array_2 = ensure_array_like(parseBlocks(message.text));
          for (let index = 0, $$length2 = each_array_2.length;index < $$length2; index++) {
            let block = each_array_2[index];
            if (block.kind === "code") {
              $$renderer2.push(`<!--[0--><div class="code-block svelte-u0c117">`);
              if (block.language)
                $$renderer2.push(`<!--[0--><span class="svelte-u0c117">${escape_html(block.language)}</span>`);
              else
                $$renderer2.push("<!--[-1-->");
              $$renderer2.push(`<!--]--> <pre class="svelte-u0c117"><code class="svelte-u0c117">${escape_html(block.content)}</code></pre></div>`);
            } else
              $$renderer2.push(`<!--[-1--><p class="svelte-u0c117">${escape_html(block.content)}</p>`);
            $$renderer2.push(`<!--]-->`);
          }
          $$renderer2.push(`<!--]--></div>`);
        } else if (message.streaming)
          $$renderer2.push(`<!--[1--><div class="typing svelte-u0c117" aria-label="Pi is thinking"><i class="svelte-u0c117"></i><i class="svelte-u0c117"></i><i class="svelte-u0c117"></i></div>`);
        else
          $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<!--]--> <!--[-->`);
        const each_array_3 = ensure_array_like(message.tools);
        for (let $$index_2 = 0, $$length2 = each_array_3.length;$$index_2 < $$length2; $$index_2++) {
          let tool = each_array_3[$$index_2];
          $$renderer2.push(`<details class="tool-card svelte-u0c117"${attr("open", tool.status === "running", true)}><summary class="svelte-u0c117"><span class="tool-icon svelte-u0c117">`);
          Wrench($$renderer2, { size: 14 });
          $$renderer2.push(`<!----></span> <span class="tool-name svelte-u0c117">${escape_html(tool.name)}</span> <span${attr_class("tool-status svelte-u0c117", undefined, {
            error: tool.status === "error",
            running: tool.status === "running"
          })}>${escape_html(tool.status === "complete" ? "Completed" : tool.status === "error" ? "Error" : "Running")}</span></summary> <div class="tool-detail svelte-u0c117"><span class="tool-label svelte-u0c117">Input</span> <pre class="svelte-u0c117">${escape_html(JSON.stringify(tool.input, null, 2))}</pre> `);
          if (tool.output)
            $$renderer2.push(`<!--[0--><span class="tool-label svelte-u0c117">Output</span> <pre class="svelte-u0c117">${escape_html(tool.output)}</pre>`);
          else
            $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<!--]--></div></details>`);
        }
        $$renderer2.push(`<!--]--> `);
        if (message.role === "assistant" && !message.streaming && message.text) {
          $$renderer2.push(`<!--[0--><button class="message-action svelte-u0c117" title="Copy response">`);
          if (copiedId === message.id) {
            $$renderer2.push("<!--[0-->");
            Check($$renderer2, { size: 13 });
          } else {
            $$renderer2.push("<!--[-1-->");
            Copy($$renderer2, { size: 13 });
          }
          $$renderer2.push(`<!--]--></button>`);
        } else if (message.queued)
          $$renderer2.push(`<!--[1--><span class="queued-label svelte-u0c117">Queued</span>`);
        else
          $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<!--]--></div></article>`);
      }
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]--> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--></div> <footer class="composer-area svelte-u0c117">`);
    if (errorMessage) {
      $$renderer2.push(`<!--[0--><div class="agent-error svelte-u0c117"><i class="svelte-u0c117">`);
      WarningCircle($$renderer2, { size: 15 });
      $$renderer2.push(`<!----></i><span class="svelte-u0c117">${escape_html(errorMessage)}</span></div>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <div${attr_class("composer svelte-u0c117", undefined, { disabled: !enabled })}><textarea${attr("placeholder", enabled ? "Ask Pi to work on this machine\u2026" : "Start the machine to chat with Pi")} rows="1"${attr("disabled", !enabled, true)} aria-label="Message Pi" class="svelte-u0c117">`);
    const $$body = escape_html(draft);
    if ($$body)
      $$renderer2.push(`${$$body}`);
    $$renderer2.push(`</textarea> `);
    $$renderer2.push(`<!--[-1--><button class="send-button svelte-u0c117"${attr("disabled", !draft.trim(), true)} aria-label="Send message">`);
    ArrowUp($$renderer2, {
      size: 16,
      weight: "bold"
    });
    $$renderer2.push(`<!----></button>`);
    $$renderer2.push(`<!--]--></div> <div class="composer-meta svelte-u0c117"><label${attr("title", currentModelLabel())} class="svelte-u0c117"><span class="svelte-u0c117">Model</span> `);
    $$renderer2.select({
      value: selectedModel,
      onchange: (event) => chooseModel(event.currentTarget.value),
      disabled: !enabled || models.length === 0 || streaming,
      class: ""
    }, ($$renderer3) => {
      if (!selectedModel) {
        $$renderer3.push("<!--[0-->");
        $$renderer3.option({
          value: "",
          class: ""
        }, ($$renderer4) => {
          $$renderer4.push(`Choose model`);
        }, "svelte-u0c117");
      } else
        $$renderer3.push("<!--[-1-->");
      $$renderer3.push(`<!--]--><!--[-->`);
      const each_array_4 = ensure_array_like(models);
      for (let $$index_4 = 0, $$length = each_array_4.length;$$index_4 < $$length; $$index_4++) {
        let model = each_array_4[$$index_4];
        $$renderer3.option({
          value: `${model.provider}/${model.id}`,
          class: ""
        }, ($$renderer4) => {
          $$renderer4.push(`${escape_html(model.name || model.id)}`);
        }, "svelte-u0c117");
      }
      $$renderer3.push(`<!--]-->`);
    }, "svelte-u0c117");
    $$renderer2.push(`</label> <label class="svelte-u0c117"><span class="svelte-u0c117">Thinking</span> `);
    $$renderer2.select({
      value: thinkingLevel,
      onchange: (event) => chooseThinking(event.currentTarget.value),
      disabled: !enabled || streaming,
      class: ""
    }, ($$renderer3) => {
      $$renderer3.push(`<!--[-->`);
      const each_array_5 = ensure_array_like(thinkingLevels);
      for (let $$index_5 = 0, $$length = each_array_5.length;$$index_5 < $$length; $$index_5++) {
        let level = each_array_5[$$index_5];
        $$renderer3.option({
          value: level,
          class: ""
        }, ($$renderer4) => {
          $$renderer4.push(`${escape_html(level)}`);
        }, "svelte-u0c117");
      }
      $$renderer3.push(`<!--]-->`);
    }, "svelte-u0c117");
    $$renderer2.push(`</label> <span class="enter-hint svelte-u0c117">Enter to send \xB7 Shift+Enter for newline</span></div></footer></section>`);
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const id = derived(() => Number(page.params.id));
    let { data } = $$props;
    let machine = derived(() => data.machine);
    let status = derived(() => data.status);
    let busy = false;
    let mainMode = "view";
    let chatWidth = 420;
    function dotClass() {
      if (!status().running)
        return "dot off";
      if (status().state === "starting")
        return "dot winding";
      if (status().state === "provisioning")
        return "dot provisioning";
      return "dot ready";
    }
    const booting = derived(() => status().state === "starting" || status().state === "provisioning");
    const live = derived(() => status().state === "ready");
    const modes = [
      {
        key: "view",
        icon: TerminalWindow,
        label: "Remote view"
      },
      {
        key: "files",
        icon: Files,
        label: "Files"
      },
      {
        key: "logs",
        icon: Scroll,
        label: "Logs"
      },
      {
        key: "analytics",
        icon: ChartLine,
        label: "Analytics"
      },
      {
        key: "settings",
        icon: GearSix,
        label: "Settings"
      }
    ];
    $$renderer2.push(`<div${attr_class("m-shell", undefined, { dragging: false })}><header class="topbar m-topbar"><div class="tb-left"><a href="/machines" class="icon-btn" aria-label="Back to machines">`);
    ArrowLeft($$renderer2, { size: 18 });
    $$renderer2.push(`<!----></a> <button class="icon-btn" aria-label="Toggle sidebar">`);
    List($$renderer2, { size: 18 });
    $$renderer2.push(`<!----></button></div> <div class="tb-center"><span${attr_class(clsx$1(dotClass()))}></span> <strong>${escape_html(machine()?.name ?? "\u2026")}</strong> <span class="meta">${escape_html(status().state)}</span></div> <div class="tb-right">`);
    if (status().running) {
      $$renderer2.push(`<!--[0--><button class="btn small"${attr("disabled", busy, true)}>`);
      Square($$renderer2, {
        size: 12,
        weight: "fill"
      });
      $$renderer2.push(`<!----> Stop</button>`);
    } else {
      $$renderer2.push(`<!--[-1--><button class="btn small primary"${attr("disabled", busy, true)}>`);
      Play($$renderer2, {
        size: 12,
        weight: "fill"
      });
      $$renderer2.push(`<!----> Start</button>`);
    }
    $$renderer2.push(`<!--]--> `);
    if (status().running) {
      $$renderer2.push(`<!--[0--><button class="btn small"${attr("disabled", busy, true)} title="Take a golden snapshot of this machine">`);
      Camera($$renderer2, { size: 12 });
      $$renderer2.push(`<!----> Snapshot</button>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <!--[-->`);
    const each_array = ensure_array_like(modes);
    for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
      let m = each_array[$$index];
      $$renderer2.push(`<button${attr_class("icon-btn", undefined, { active: mainMode === m.key })}${attr("aria-label", m.label)}${attr("title", m.label)}>`);
      if (m.icon) {
        $$renderer2.push("<!--[-->");
        m.icon($$renderer2, { size: 18 });
        $$renderer2.push("<!--]-->");
      } else {
        $$renderer2.push("<!--[!-->");
        $$renderer2.push("<!--]-->");
      }
      $$renderer2.push(`</button>`);
    }
    $$renderer2.push(`<!--]--></div></header> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <div class="m-body">`);
    $$renderer2.push(`<!--[0--><aside class="chat-sidebar"${attr_style(`width: ${stringify(chatWidth)}px; min-width: 20rem;`)}><div class="chat-tabs"><button${attr_class("", undefined, { active: true })}>Chat</button> <button${attr_class("", undefined, { active: false })}>Console</button></div> `);
    if (booting())
      $$renderer2.push(`<!--[0--><div class="chat-body placeholder"><div class="skeleton" style="height: 1rem; width: 65%;"></div> <div class="skeleton" style="height: 3.5rem;"></div> <div class="skeleton" style="height: 1rem; width: 45%;"></div> <p class="hint">${escape_html(status().state === "starting" ? "Creating the droplet\u2026" : "Provisioning \u2014 waiting for the daemon to dial home\u2026")}</p></div>`);
    else {
      $$renderer2.push(`<!--[1--><!---->`);
      AgentChat($$renderer2, {
        machineId: id(),
        machineName: machine()?.name ?? "builder",
        enabled: live()
      });
      $$renderer2.push(`<!---->`);
    }
    $$renderer2.push(`<!--]--></aside> <div class="resizer" role="separator" aria-orientation="vertical"></div>`);
    $$renderer2.push(`<!--]--> <section class="main-area">`);
    $$renderer2.push("<!--[0-->");
    if (booting())
      $$renderer2.push(`<!--[0--><div class="webrtc-stage placeholder"><div class="skeleton" style="height: 2rem; width: 50%;"></div> <div class="skeleton" style="height: 55%;"></div> <p class="hint">${escape_html(status().state === "starting" ? "Creating the droplet\u2026" : "Provisioning \u2014 the stream appears once the daemon confirms the machine is ready.")}</p></div>`);
    else {
      $$renderer2.push(`<!--[-1--><div class="webrtc-stage placeholder"><p class="hint">`);
      if (live())
        $$renderer2.push(`<!--[0-->WebRTC app stream lands in Phase 3 (cage + screencopy + WHEP).`);
      else
        $$renderer2.push(`<!--[-1-->Machine is off \u2014 press Start, then the stream appears here.`);
      $$renderer2.push(`<!--]--></p> `);
      if (status().ip)
        $$renderer2.push(`<!--[0--><p class="hint">IP: <code>${escape_html(status().ip)}</code></p>`);
      else
        $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]-->`);
    $$renderer2.push(`<!--]--></section></div></div>`);
  });
}
export {
  _page as default
};

//# debugId=5D6C177C6784123764756E2164756E21
