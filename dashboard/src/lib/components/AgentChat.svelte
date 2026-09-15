<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    ArrowDown,
    ArrowUp,
    Brain,
    Check,
    ChatCircleDots,
    Copy,
    Plus,
    Sparkle,
    Stop,
    WarningCircle,
    Wrench,
  } from 'phosphor-svelte';

  interface ToolRun {
    id: string;
    name: string;
    status: 'running' | 'complete' | 'error';
    input: unknown;
    output: string;
  }

  interface ChatMessage {
    id: string;
    role: 'user' | 'assistant';
    text: string;
    thinking: string;
    tools: ToolRun[];
    streaming?: boolean;
    queued?: boolean;
  }

  interface ModelOption {
    provider: string;
    id: string;
    name?: string;
  }

  interface TextBlock {
    kind: 'text' | 'code';
    content: string;
    language?: string;
  }

  let {
    machineId,
    machineName,
    enabled = true,
  }: { machineId: number; machineName: string; enabled?: boolean } = $props();

  let messages = $state.raw<ChatMessage[]>([]);
  let models = $state.raw<ModelOption[]>([]);
  let thinkingLevels = $state.raw<string[]>(['off']);
  let selectedModel = $state('');
  let thinkingLevel = $state('off');
  let draft = $state('');
  let streaming = $state(false);
  let connected = $state(false);
  let reconnecting = $state(false);
  let sending = $state(false);
  let errorMessage = $state('');
  let queueCount = $state(0);
  let copiedId = $state('');
  let nearBottom = $state(true);
  let messageCounter = 0;
  let eventSource: EventSource | undefined;
  let conversationEl: HTMLDivElement;

  const currentModelLabel = $derived.by(() => {
    const match = models.find((model) => `${model.provider}/${model.id}` === selectedModel);
    return match?.name || selectedModel || 'Choose model';
  });

  const suggestions = [
    'Inspect the Rust workspace and summarize its architecture',
    'Run the test suite and fix the first failure',
    'Find the slowest part of the current build',
  ];

  function makeId(prefix: string): string {
    messageCounter += 1;
    return `${prefix}-${Date.now()}-${messageCounter}`;
  }

  function contentText(content: unknown, type = 'text'): string {
    if (typeof content === 'string') return type === 'text' ? content : '';
    if (!Array.isArray(content)) return '';
    return content
      .filter((part) => part && typeof part === 'object' && (part as Record<string, unknown>).type === type)
      .map((part) => String((part as Record<string, unknown>).text ?? ''))
      .join('');
  }

  function resultText(result: unknown): string {
    if (!result || typeof result !== 'object') return '';
    return contentText((result as Record<string, unknown>).content);
  }

  function parseBlocks(text: string): TextBlock[] {
    const blocks: TextBlock[] = [];
    const expression = /```([^\n]*)\n([\s\S]*?)```/g;
    let cursor = 0;
    for (const match of text.matchAll(expression)) {
      const index = match.index ?? 0;
      if (index > cursor) blocks.push({ kind: 'text', content: text.slice(cursor, index) });
      blocks.push({ kind: 'code', language: match[1].trim(), content: match[2].replace(/\n$/, '') });
      cursor = index + match[0].length;
    }
    if (cursor < text.length) blocks.push({ kind: 'text', content: text.slice(cursor) });
    return blocks.length ? blocks : [{ kind: 'text', content: text }];
  }

  function toolsFromContent(content: unknown): ToolRun[] {
    if (!Array.isArray(content)) return [];
    return content.flatMap((part) => {
      if (!part || typeof part !== 'object') return [];
      const value = part as Record<string, unknown>;
      if (value.type !== 'toolCall') return [];
      return [{
        id: String(value.id ?? makeId('tool')),
        name: String(value.name ?? value.toolName ?? 'tool'),
        status: 'running' as const,
        input: value.arguments ?? value.args ?? {},
        output: '',
      }];
    });
  }

  function hydrateMessages(rawMessages: unknown): void {
    if (!Array.isArray(rawMessages)) return;
    const hydrated: ChatMessage[] = [];
    for (const [index, raw] of rawMessages.entries()) {
      if (!raw || typeof raw !== 'object') continue;
      const message = raw as Record<string, unknown>;
      const role = message.role;
      if (role === 'user' || role === 'assistant') {
        hydrated.push({
          id: String(message.id ?? `${role}-${message.timestamp ?? index}`),
          role,
          text: contentText(message.content),
          thinking: contentText(message.content, 'thinking'),
          tools: role === 'assistant' ? toolsFromContent(message.content) : [],
        });
      } else if (role === 'toolResult') {
        const callId = String(message.toolCallId ?? '');
        for (let messageIndex = hydrated.length - 1; messageIndex >= 0; messageIndex -= 1) {
          const toolIndex = hydrated[messageIndex].tools.findIndex((tool) => tool.id === callId);
          if (toolIndex >= 0) {
            hydrated[messageIndex].tools[toolIndex] = {
              ...hydrated[messageIndex].tools[toolIndex],
              status: message.isError ? 'error' : 'complete',
              output: contentText(message.content),
            };
            break;
          }
        }
      }
    }
    messages = hydrated;
    void scrollToBottom(true);
  }

  function activeAssistant(): ChatMessage {
    const last = messages.at(-1);
    if (last?.role === 'assistant' && last.streaming) return last;
    const assistant: ChatMessage = {
      id: makeId('assistant'),
      role: 'assistant',
      text: '',
      thinking: '',
      tools: [],
      streaming: true,
    };
    messages = [...messages, assistant];
    return assistant;
  }

  function updateMessage(id: string, update: (message: ChatMessage) => ChatMessage): void {
    messages = messages.map((message) => (message.id === id ? update(message) : message));
  }

  function updateTool(callId: string, update: (tool: ToolRun) => ToolRun): void {
    const assistant = activeAssistant();
    updateMessage(assistant.id, (message) => {
      const existing = message.tools.find((tool) => tool.id === callId);
      const tools = existing
        ? message.tools.map((tool) => (tool.id === callId ? update(tool) : tool))
        : [...message.tools, update({ id: callId, name: 'tool', status: 'running', input: {}, output: '' })];
      return { ...message, tools };
    });
  }

  function processResponse(event: Record<string, unknown>): void {
    const command = String(event.command ?? '');
    if (event.success === false) {
      errorMessage = String(event.error ?? `${command || 'Pi command'} failed.`);
      sending = false;
      return;
    }
    const data = (event.data ?? {}) as Record<string, unknown>;
    if (command === 'get_messages') hydrateMessages(data.messages);
    if (command === 'get_state') {
      const model = data.model as Record<string, unknown> | null;
      if (model) selectedModel = `${model.provider}/${model.id}`;
      thinkingLevel = String(data.thinkingLevel ?? 'off');
      streaming = Boolean(data.isStreaming);
    }
    if (command === 'get_available_models' && Array.isArray(data.models)) {
      models = (data.models as Record<string, unknown>[])
        .map((model) => ({
          provider: String(model.provider ?? ''),
          id: String(model.id ?? ''),
          name: String(model.name ?? model.id ?? ''),
        }))
        .filter((model) => model.provider && model.id);
    }
    if (command === 'get_available_thinking_levels' && Array.isArray(data.levels)) {
      thinkingLevels = data.levels.map(String);
    }
    if (command === 'new_session' && !(data.cancelled as boolean)) {
      messages = [];
      queueCount = 0;
    }
    sending = false;
  }

  function processEvent(rawEvent: unknown): void {
    if (!rawEvent || typeof rawEvent !== 'object') return;
    const event = rawEvent as Record<string, unknown>;
    const type = String(event.type ?? '');

    if (type === 'response') {
      processResponse(event);
    } else if (type === 'agent_start') {
      streaming = true;
      sending = false;
    } else if (type === 'agent_settled' || (type === 'agent_end' && !event.willRetry)) {
      streaming = false;
      sending = false;
      messages = messages.map((message) => ({ ...message, streaming: false, queued: false }));
    } else if (type === 'message_start') {
      const message = event.message as Record<string, unknown> | undefined;
      if (message?.role === 'assistant') activeAssistant();
    } else if (type === 'message_update') {
      const deltaEvent = event.assistantMessageEvent as Record<string, unknown> | undefined;
      if (!deltaEvent) return;
      const assistant = activeAssistant();
      if (deltaEvent.type === 'text_delta') {
        updateMessage(assistant.id, (message) => ({
          ...message,
          text: message.text + String(deltaEvent.delta ?? ''),
        }));
      } else if (deltaEvent.type === 'thinking_delta') {
        updateMessage(assistant.id, (message) => ({
          ...message,
          thinking: message.thinking + String(deltaEvent.delta ?? ''),
        }));
      }
    } else if (type === 'message_end') {
      const finalMessage = event.message as Record<string, unknown> | undefined;
      if (finalMessage?.role === 'assistant') {
        const assistant = activeAssistant();
        updateMessage(assistant.id, (message) => ({
          ...message,
          text: contentText(finalMessage.content) || message.text,
          thinking: contentText(finalMessage.content, 'thinking') || message.thinking,
          streaming: false,
        }));
      }
    } else if (type === 'tool_execution_start') {
      const callId = String(event.toolCallId ?? makeId('tool'));
      updateTool(callId, (tool) => ({
        ...tool,
        name: String(event.toolName ?? tool.name),
        status: 'running',
        input: event.args ?? {},
      }));
    } else if (type === 'tool_execution_update') {
      const callId = String(event.toolCallId ?? '');
      updateTool(callId, (tool) => ({
        ...tool,
        name: String(event.toolName ?? tool.name),
        output: resultText(event.partialResult),
      }));
    } else if (type === 'tool_execution_end') {
      const callId = String(event.toolCallId ?? '');
      updateTool(callId, (tool) => ({
        ...tool,
        name: String(event.toolName ?? tool.name),
        status: event.isError ? 'error' : 'complete',
        output: resultText(event.result) || tool.output,
      }));
    } else if (type === 'queue_update') {
      const steering = Array.isArray(event.steering) ? event.steering.length : 0;
      const followUp = Array.isArray(event.followUp) ? event.followUp.length : 0;
      queueCount = steering + followUp;
    } else if (type === 'auto_retry_start') {
      errorMessage = `Pi is retrying in ${Math.round(Number(event.delayMs ?? 0) / 1000)}s: ${String(event.errorMessage ?? '')}`;
    } else if (type === 'bridge_error' || type === 'extension_error') {
      errorMessage = String(event.message ?? event.error ?? 'The agent bridge reported an error.');
    } else if (type === 'bridge_status') {
      connected = event.state === 'connected';
    }
    void scrollToBottom();
  }

  function connect(): void {
    if (!enabled) return;
    eventSource = new EventSource(`/api/machines/${machineId}/agent`);
    eventSource.onopen = () => {
      connected = true;
      reconnecting = false;
      errorMessage = '';
    };
    eventSource.onmessage = (message) => {
      try {
        const envelope = JSON.parse(message.data) as { event?: unknown };
        processEvent(envelope.event);
      } catch {
        errorMessage = 'The daemon sent an event the dashboard could not read.';
      }
    };
    eventSource.addEventListener('reset', () => {
      void sendCommand({ type: 'get_messages' });
    });
    eventSource.onerror = () => {
      connected = false;
      reconnecting = true;
    };
  }

  async function sendCommand(command: Record<string, unknown>): Promise<boolean> {
    errorMessage = '';
    try {
      const response = await fetch(`/api/machines/${machineId}/agent`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(command),
      });
      const body = (await response.json().catch(() => ({}))) as { error?: string };
      if (!response.ok) throw new Error(body.error ?? `Request failed (${response.status}).`);
      return true;
    } catch (error) {
      errorMessage = (error as Error).message;
      sending = false;
      return false;
    }
  }

  async function submit(text = draft): Promise<void> {
    const prompt = text.trim();
    if (!prompt || sending || !enabled) return;
    const wasStreaming = streaming;
    sending = true;
    draft = '';
    const optimisticId = makeId('user');
    messages = [...messages, {
      id: optimisticId,
      role: 'user',
      text: prompt,
      thinking: '',
      tools: [],
      queued: wasStreaming,
    }];
    await scrollToBottom(true);
    const accepted = await sendCommand({
      type: 'prompt',
      message: prompt,
      ...(wasStreaming ? { streamingBehavior: 'followUp' } : {}),
    });
    if (!accepted) {
      updateMessage(optimisticId, (message) => ({ ...message, queued: false }));
      draft = prompt;
    }
  }

  async function abort(): Promise<void> {
    await sendCommand({ type: 'clear_queue' });
    await sendCommand({ type: 'abort' });
    queueCount = 0;
  }

  async function startNewSession(): Promise<void> {
    if (streaming) await abort();
    await sendCommand({ type: 'new_session' });
  }

  async function chooseModel(value: string): Promise<void> {
    const separator = value.indexOf('/');
    if (separator < 1) return;
    const previous = selectedModel;
    selectedModel = value;
    const accepted = await sendCommand({
      type: 'set_model',
      provider: value.slice(0, separator),
      modelId: value.slice(separator + 1),
    });
    if (!accepted) selectedModel = previous;
    else await sendCommand({ type: 'get_available_thinking_levels' });
  }

  async function chooseThinking(value: string): Promise<void> {
    const previous = thinkingLevel;
    thinkingLevel = value;
    if (!(await sendCommand({ type: 'set_thinking_level', level: value }))) {
      thinkingLevel = previous;
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void submit();
    }
  }

  function handleScroll(): void {
    const distance = conversationEl.scrollHeight - conversationEl.scrollTop - conversationEl.clientHeight;
    nearBottom = distance < 96;
  }

  async function scrollToBottom(force = false): Promise<void> {
    if (!force && !nearBottom) return;
    await tick();
    conversationEl?.scrollTo({ top: conversationEl.scrollHeight, behavior: force ? 'smooth' : 'auto' });
  }

  async function copyMessage(message: ChatMessage): Promise<void> {
    await navigator.clipboard.writeText(message.text);
    copiedId = message.id;
    window.setTimeout(() => {
      if (copiedId === message.id) copiedId = '';
    }, 1500);
  }

  onMount(() => {
    connect();
    return () => eventSource?.close();
  });
</script>

<section class="agent-chat" aria-label={`Pi agent chat for ${machineName}`}>
  <header class="agent-header">
    <div class="agent-identity">
      <span class="agent-avatar"><Sparkle size={16} weight="fill" /></span>
      <div>
        <strong>Pi Agent</strong>
        <span class="connection-line">
          <i class:online={connected}></i>
          {#if connected}Connected{:else if reconnecting}Reconnecting…{:else}Offline{/if}
        </span>
      </div>
    </div>
    <button class="new-chat" onclick={startNewSession} title="Start a new conversation" disabled={!enabled}>
      <Plus size={15} /> New
    </button>
  </header>

  <div class="conversation" bind:this={conversationEl} onscroll={handleScroll}>
    {#if messages.length === 0}
      <div class="empty-chat">
        <span class="empty-icon"><ChatCircleDots size={27} weight="duotone" /></span>
        <h2>Build with Pi</h2>
        <p>Ask the agent to inspect, edit, compile, or test the workspace on this machine.</p>
        <div class="suggestions">
          {#each suggestions as suggestion (suggestion)}
            <button onclick={() => submit(suggestion)} disabled={!enabled || sending}>{suggestion}</button>
          {/each}
        </div>
      </div>
    {:else}
      <div class="message-list" aria-live="polite">
        {#each messages as message (message.id)}
          <article class:from-user={message.role === 'user'} class="chat-message">
            {#if message.role === 'assistant'}
              <span class="message-avatar"><Sparkle size={14} weight="fill" /></span>
            {/if}
            <div class="message-stack">
              {#if message.role === 'assistant' && message.thinking}
                <details class="thinking-block">
                  <summary><Brain size={14} /> Reasoning</summary>
                  <p>{message.thinking}</p>
                </details>
              {/if}

              {#if message.text}
                <div class="message-content">
                  {#each parseBlocks(message.text) as block, index (`${message.id}-${index}`)}
                    {#if block.kind === 'code'}
                      <div class="code-block">
                        {#if block.language}<span>{block.language}</span>{/if}
                        <pre><code>{block.content}</code></pre>
                      </div>
                    {:else}
                      <p>{block.content}</p>
                    {/if}
                  {/each}
                </div>
              {:else if message.streaming}
                <div class="typing" aria-label="Pi is thinking"><i></i><i></i><i></i></div>
              {/if}

              {#each message.tools as tool (tool.id)}
                <details class="tool-card" open={tool.status === 'running'}>
                  <summary>
                    <span class="tool-icon"><Wrench size={14} /></span>
                    <span class="tool-name">{tool.name}</span>
                    <span class:error={tool.status === 'error'} class:running={tool.status === 'running'} class="tool-status">
                      {tool.status === 'complete' ? 'Completed' : tool.status === 'error' ? 'Error' : 'Running'}
                    </span>
                  </summary>
                  <div class="tool-detail">
                    <span class="tool-label">Input</span>
                    <pre>{JSON.stringify(tool.input, null, 2)}</pre>
                    {#if tool.output}
                      <span class="tool-label">Output</span>
                      <pre>{tool.output}</pre>
                    {/if}
                  </div>
                </details>
              {/each}

              {#if message.role === 'assistant' && !message.streaming && message.text}
                <button class="message-action" onclick={() => copyMessage(message)} title="Copy response">
                  {#if copiedId === message.id}<Check size={13} />{:else}<Copy size={13} />{/if}
                </button>
              {:else if message.queued}
                <span class="queued-label">Queued</span>
              {/if}
            </div>
          </article>
        {/each}
      </div>
    {/if}

    {#if !nearBottom}
      <button class="scroll-bottom" onclick={() => scrollToBottom(true)} aria-label="Scroll to latest message">
        <ArrowDown size={16} />
      </button>
    {/if}
  </div>

  <footer class="composer-area">
    {#if errorMessage}
      <div class="agent-error"><i><WarningCircle size={15} /></i><span>{errorMessage}</span></div>
    {/if}
    {#if queueCount > 0}<div class="queue-note">{queueCount} follow-up{queueCount === 1 ? '' : 's'} queued</div>{/if}
    <div class:disabled={!enabled} class="composer">
      <textarea
        bind:value={draft}
        onkeydown={handleKeydown}
        placeholder={enabled ? 'Ask Pi to work on this machine…' : 'Start the machine to chat with Pi'}
        rows="1"
        disabled={!enabled}
        aria-label="Message Pi"
      ></textarea>
      {#if streaming && !draft.trim()}
        <button class="send-button stop-button" onclick={abort} aria-label="Stop Pi">
          <Stop size={14} weight="fill" />
        </button>
      {:else}
        <button class="send-button" onclick={() => submit()} disabled={!draft.trim() || sending || !enabled} aria-label="Send message">
          <ArrowUp size={16} weight="bold" />
        </button>
      {/if}
    </div>
    <div class="composer-meta">
      <label title={currentModelLabel}>
        <span>Model</span>
        <select value={selectedModel} onchange={(event) => chooseModel(event.currentTarget.value)} disabled={!enabled || models.length === 0 || streaming}>
          {#if !selectedModel}<option value="">Choose model</option>{/if}
          {#each models as model (`${model.provider}/${model.id}`)}
            <option value={`${model.provider}/${model.id}`}>{model.name || model.id}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>Thinking</span>
        <select value={thinkingLevel} onchange={(event) => chooseThinking(event.currentTarget.value)} disabled={!enabled || streaming}>
          {#each thinkingLevels as level (level)}<option value={level}>{level}</option>{/each}
        </select>
      </label>
      <span class="enter-hint">Enter to send · Shift+Enter for newline</span>
    </div>
  </footer>
</section>

<style>
  .agent-chat { display: flex; flex-direction: column; min-height: 0; height: 100%; background: var(--card); }
  .agent-header { display: flex; align-items: center; justify-content: space-between; min-height: 3.5rem; padding: .65rem .8rem; border-bottom: var(--border-width) solid var(--border); }
  .agent-identity { display: flex; align-items: center; gap: .65rem; min-width: 0; }
  .agent-identity > div { display: flex; flex-direction: column; line-height: 1.2; }
  .agent-identity strong { font-size: .875rem; }
  .agent-avatar, .message-avatar, .empty-icon { display: inline-flex; align-items: center; justify-content: center; color: var(--primary); background: color-mix(in srgb, var(--powder-blue) 13%, transparent); border: var(--border-width) solid color-mix(in srgb, var(--powder-blue) 22%, transparent); }
  .agent-avatar { width: 2rem; height: 2rem; border-radius: .65rem; }
  .connection-line { display: flex; align-items: center; gap: .3rem; color: var(--muted-foreground); font-size: .7rem; margin-top: .2rem; }
  .connection-line i { width: .4rem; height: .4rem; border-radius: 50%; background: var(--muted-foreground); }
  .connection-line i.online { background: var(--success); box-shadow: 0 0 .45rem color-mix(in srgb, var(--success) 55%, transparent); }
  .new-chat { display: inline-flex; align-items: center; gap: .3rem; border: var(--border-width) solid var(--border); background: transparent; color: var(--muted-foreground); border-radius: .45rem; padding: .38rem .52rem; font: inherit; font-size: .75rem; cursor: pointer; }
  .new-chat:hover { color: var(--foreground); background: var(--muted); }
  .conversation { position: relative; flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: thin; }
  .empty-chat { min-height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 2rem 1.25rem; }
  .empty-icon { width: 3rem; height: 3rem; border-radius: 1rem; margin-bottom: .8rem; }
  .empty-chat h2 { margin: 0; font-size: 1.1rem; }
  .empty-chat p { color: var(--muted-foreground); font-size: .8rem; max-width: 17rem; margin: .4rem 0 1.1rem; }
  .suggestions { display: flex; flex-direction: column; gap: .45rem; width: 100%; max-width: 18rem; }
  .suggestions button { text-align: left; border: var(--border-width) solid var(--border); border-radius: .65rem; background: color-mix(in srgb, var(--muted) 55%, transparent); color: var(--foreground); padding: .65rem .75rem; font: inherit; font-size: .76rem; line-height: 1.35; cursor: pointer; }
  .suggestions button:hover { border-color: var(--cool-steel); background: var(--muted); }
  .message-list { display: flex; flex-direction: column; gap: 1.15rem; padding: 1rem .9rem 1.5rem; }
  .chat-message { display: flex; align-items: flex-start; gap: .55rem; max-width: 100%; }
  .chat-message.from-user { justify-content: flex-end; padding-left: 12%; }
  .message-avatar { flex: 0 0 auto; width: 1.7rem; height: 1.7rem; border-radius: .55rem; margin-top: .1rem; }
  .message-stack { min-width: 0; max-width: calc(100% - 2.25rem); display: flex; flex-direction: column; align-items: flex-start; gap: .45rem; }
  .from-user .message-stack { align-items: flex-end; max-width: 100%; }
  .message-content { min-width: 0; color: var(--foreground); font-size: .84rem; line-height: 1.62; }
  .from-user .message-content { background: color-mix(in srgb, var(--powder-blue) 16%, var(--muted)); border: var(--border-width) solid color-mix(in srgb, var(--powder-blue) 20%, transparent); border-radius: .85rem .85rem .25rem .85rem; padding: .58rem .75rem; }
  .message-content p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  .message-content p + p { margin-top: .6rem; }
  .code-block { overflow: hidden; margin: .55rem 0; border: var(--border-width) solid var(--border); border-radius: .6rem; background: color-mix(in srgb, var(--background) 86%, black); }
  .code-block > span { display: block; padding: .3rem .55rem; border-bottom: var(--border-width) solid var(--border); color: var(--muted-foreground); font-family: var(--font-mono); font-size: .66rem; }
  .code-block pre { overflow-x: auto; margin: 0; padding: .7rem; }
  .code-block code { display: block; padding: 0; background: transparent; color: var(--foreground); font-size: .74rem; line-height: 1.55; }
  .thinking-block { width: 100%; color: var(--muted-foreground); font-size: .73rem; }
  .thinking-block summary { display: flex; align-items: center; gap: .35rem; cursor: pointer; user-select: none; }
  .thinking-block p { margin: .45rem 0 0; padding-left: 1.15rem; white-space: pre-wrap; line-height: 1.5; }
  .tool-card { width: 100%; border: var(--border-width) solid var(--border); border-radius: .6rem; background: color-mix(in srgb, var(--muted) 45%, transparent); overflow: hidden; }
  .tool-card summary { display: flex; align-items: center; gap: .45rem; padding: .52rem .6rem; cursor: pointer; list-style: none; font-size: .73rem; }
  .tool-card summary::-webkit-details-marker { display: none; }
  .tool-icon { display: inline-flex; color: var(--muted-foreground); }
  .tool-name { flex: 1; font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; }
  .tool-status { color: var(--muted-foreground); font-size: .66rem; }
  .tool-status.running { color: var(--primary); }
  .tool-status.error { color: var(--destructive); }
  .tool-detail { border-top: var(--border-width) solid var(--border); padding: .55rem; }
  .tool-label { display: block; margin: .15rem 0 .3rem; color: var(--muted-foreground); font-size: .65rem; text-transform: uppercase; letter-spacing: .04em; }
  .tool-detail pre { max-height: 13rem; overflow: auto; white-space: pre-wrap; word-break: break-word; margin: 0 0 .5rem; padding: .5rem; border-radius: .4rem; background: color-mix(in srgb, var(--background) 78%, black); color: var(--foreground); font: .68rem/1.45 var(--font-mono); }
  .typing { display: flex; gap: .25rem; padding: .45rem 0; }
  .typing i { width: .35rem; height: .35rem; border-radius: 50%; background: var(--muted-foreground); animation: pulse 1.2s infinite ease-in-out; }
  .typing i:nth-child(2) { animation-delay: .15s; } .typing i:nth-child(3) { animation-delay: .3s; }
  @keyframes pulse { 0%, 70%, 100% { opacity: .3; transform: translateY(0); } 35% { opacity: 1; transform: translateY(-.15rem); } }
  .message-action { display: inline-flex; align-items: center; justify-content: center; width: 1.7rem; height: 1.55rem; padding: 0; border: 0; border-radius: .35rem; background: transparent; color: var(--muted-foreground); cursor: pointer; opacity: .6; }
  .chat-message:hover .message-action { opacity: 1; } .message-action:hover { background: var(--muted); color: var(--foreground); }
  .queued-label { color: var(--muted-foreground); font-size: .66rem; }
  .scroll-bottom { position: sticky; left: 50%; bottom: .6rem; display: flex; align-items: center; justify-content: center; width: 2rem; height: 2rem; margin-left: calc(50% - 1rem); border: var(--border-width) solid var(--border); border-radius: 50%; background: var(--card); color: var(--foreground); box-shadow: var(--shadow-lift); cursor: pointer; }
  .composer-area { padding: .55rem .7rem .65rem; border-top: var(--border-width) solid var(--border); background: color-mix(in srgb, var(--card) 94%, var(--background)); }
  .agent-error { display: flex; gap: .4rem; align-items: flex-start; padding: .45rem .5rem; margin-bottom: .45rem; border-radius: .45rem; color: var(--destructive-foreground); background: color-mix(in srgb, var(--destructive) 18%, transparent); font-size: .7rem; }
  .agent-error i { display: inline-flex; flex: 0 0 auto; margin-top: .08rem; }
  .queue-note { margin: 0 0 .35rem .2rem; color: var(--muted-foreground); font-size: .66rem; }
  .composer { display: flex; align-items: flex-end; gap: .35rem; padding: .42rem; border: var(--border-width) solid color-mix(in srgb, var(--powder-blue) 28%, var(--border)); border-radius: .8rem; background: var(--background); box-shadow: 0 0 0 .12rem transparent; transition: border-color .15s, box-shadow .15s; }
  .composer:focus-within { border-color: var(--ring); box-shadow: 0 0 0 .12rem color-mix(in srgb, var(--ring) 15%, transparent); }
  .composer.disabled { opacity: .55; }
  textarea { flex: 1; min-height: 2rem; max-height: 8rem; resize: vertical; border: 0; outline: 0; background: transparent; color: var(--foreground); padding: .38rem .35rem; font: inherit; font-size: .82rem; line-height: 1.4; }
  textarea::placeholder { color: var(--muted-foreground); }
  .send-button { flex: 0 0 auto; display: flex; align-items: center; justify-content: center; width: 2rem; height: 2rem; padding: 0; border: 0; border-radius: .58rem; background: var(--primary); color: var(--primary-foreground); cursor: pointer; }
  .send-button:disabled { opacity: .35; cursor: default; } .stop-button { background: var(--foreground); color: var(--background); }
  .composer-meta { display: flex; align-items: center; gap: .3rem; margin-top: .4rem; min-width: 0; }
  .composer-meta label { display: flex; align-items: center; min-width: 0; }
  .composer-meta label > span { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); }
  .composer-meta select { max-width: 8.5rem; border: 0; background: transparent; color: var(--muted-foreground); font: inherit; font-size: .64rem; text-overflow: ellipsis; cursor: pointer; }
  .composer-meta select option { background: var(--card); color: var(--foreground); }
  .enter-hint { margin-left: auto; color: var(--muted-foreground); font-size: .58rem; white-space: nowrap; }
  @media (max-width: 52rem) { .enter-hint { display: none; } }
  @media (prefers-reduced-motion: reduce) { .typing i { animation: none; } }
</style>
