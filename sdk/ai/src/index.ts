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
  readonly streamOptions?: StreamOptions;
}

/** OpenAI-compatible stream controls. */
export interface StreamOptions {
  readonly includeUsage: boolean;
}

/** Host-mediated response stream; events have passed the signed route schema. */
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

/** Token accounting emitted by a terminal provider event. */
export interface ChatUsage {
  readonly promptTokens: number;
  readonly completionTokens: number;
  readonly totalTokens: number;
}

/** Provider error carried in a terminal stream event. */
export interface ChatError {
  readonly message: string;
  readonly errorType?: string;
  readonly param?: string;
  readonly code?: string;
}

/** Typed event projection for incremental output and terminal provider events. */
export type AiStreamEvent =
  | { readonly kind: "delta"; readonly delta: ChatDelta }
  | { readonly kind: "usage"; readonly usage: ChatUsage }
  | { readonly kind: "error"; readonly error: ChatError };

/** Provider-neutral AI client. API keys remain protected host configuration. */
export class AiClient {
  public constructor(private readonly transport: AiTransport) {}

  public complete(request: ChatRequest): Promise<unknown> {
    return this.transport.complete({ ...request, stream: false });
  }

  public stream(request: ChatRequest): AsyncIterable<AiStreamEvent> {
    const source = this.transport.stream({ ...request, stream: true });
    return this.events(source);
  }

  private async *events(source: AiChunkStream): AsyncGenerator<AiStreamEvent> {
    for await (const value of source) yield parseEvent(value);
  }
}

function parseEvent(value: unknown): AiStreamEvent {
  if (!value || typeof value !== "object") throw new Error("ai payload projection invalid");
  const error = (value as { error?: unknown }).error;
  if (error && typeof error === "object") {
    const message = (error as { message?: unknown }).message;
    if (typeof message !== "string" || message.length === 0) throw new Error("ai payload projection invalid");
    return {
      kind: "error",
      error: {
        message,
        errorType: typeof (error as { type?: unknown }).type === "string" ? (error as { type: string }).type : undefined,
        param: typeof (error as { param?: unknown }).param === "string" ? (error as { param: string }).param : undefined,
        code: typeof (error as { code?: unknown }).code === "string" ? (error as { code: string }).code : undefined,
      },
    };
  }
  const choices = (value as { choices?: unknown }).choices;
  const usage = (value as { usage?: unknown }).usage;
  if (Array.isArray(choices) && choices.length === 0 && usage && typeof usage === "object") {
    const promptTokens = nonNegativeInteger((usage as { prompt_tokens?: unknown }).prompt_tokens);
    const completionTokens = nonNegativeInteger((usage as { completion_tokens?: unknown }).completion_tokens);
    const totalTokens = nonNegativeInteger((usage as { total_tokens?: unknown }).total_tokens);
    if (promptTokens === undefined || completionTokens === undefined || totalTokens === undefined) {
      throw new Error("ai payload projection invalid");
    }
    return { kind: "usage", usage: { promptTokens, completionTokens, totalTokens } };
  }
  if (!Array.isArray(choices) || choices.length === 0 || !choices[0] || typeof choices[0] !== "object") {
    throw new Error("ai payload projection invalid");
  }
  const choice = choices[0] as { index?: unknown; delta?: unknown; finish_reason?: unknown };
  const delta = choice.delta;
  const text = delta && typeof delta === "object" && typeof (delta as { content?: unknown }).content === "string"
    ? (delta as { content: string }).content
    : undefined;
  return {
    kind: "delta",
    delta: {
      choiceIndex: typeof choice.index === "number" ? choice.index : 0,
      text,
      finishReason: typeof choice.finish_reason === "string" || choice.finish_reason === null ? choice.finish_reason : undefined,
    },
  };
}

function nonNegativeInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : undefined;
}
