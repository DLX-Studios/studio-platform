/** OpenAI-compatible chat message. */
export interface ChatMessage {
  readonly role: "system" | "user" | "assistant";
  readonly content: string;
}

/** OpenAI-compatible request body. */
export interface ChatRequest {
  readonly model: string;
  readonly messages: readonly ChatMessage[];
  readonly stream?: boolean;
  readonly temperature?: number;
}

/** Host-mediated response stream; chunks have passed the signed route schema. */
export interface AiChunkStream {
  readonly [Symbol.asyncIterator]: () => AsyncIterator<unknown>;
}

/** Restricted transport contract for an OpenAI-compatible provider. */
export interface AiTransport {
  complete(request: ChatRequest): Promise<unknown>;
  stream(request: ChatRequest): AiChunkStream;
}

/** Normalized incremental assistant output. */
export interface ChatDelta {
  readonly choiceIndex: number;
  readonly text?: string;
  readonly finishReason?: string | null;
}

/** Provider-neutral AI client. API keys remain protected host configuration. */
export class AiClient {
  public constructor(private readonly transport: AiTransport) {}

  public complete(request: ChatRequest): Promise<unknown> {
    return this.transport.complete({ ...request, stream: false });
  }

  public stream(request: ChatRequest): AsyncIterable<ChatDelta> {
    const source = this.transport.stream({ ...request, stream: true });
    return this.deltas(source);
  }

  private async *deltas(source: AiChunkStream): AsyncGenerator<ChatDelta> {
    for await (const value of source) yield parseChunk(value);
  }
}

function parseChunk(value: unknown): ChatDelta {
  if (!value || typeof value !== "object") throw new Error("ai payload projection invalid");
  const choices = (value as { choices?: unknown }).choices;
  if (!Array.isArray(choices) || choices.length === 0 || !choices[0] || typeof choices[0] !== "object") {
    throw new Error("ai payload projection invalid");
  }
  const choice = choices[0] as { index?: unknown; delta?: unknown; finish_reason?: unknown };
  const delta = choice.delta;
  const text = delta && typeof delta === "object" && typeof (delta as { content?: unknown }).content === "string"
    ? (delta as { content: string }).content
    : undefined;
  return {
    choiceIndex: typeof choice.index === "number" ? choice.index : 0,
    text,
    finishReason: typeof choice.finish_reason === "string" || choice.finish_reason === null ? choice.finish_reason : undefined,
  };
}
