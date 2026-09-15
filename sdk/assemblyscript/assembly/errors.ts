/** Stable SDK error that never embeds raw protocol payloads. */
export class StudioError extends Error {
  constructor(public readonly code: string, message: string) {
    super(message);
  }
}

export function validationError(code: string): StudioError {
  if (code.length == 0) return new StudioError("diagnostic_invalid", "Studio operation invalid");
  return new StudioError(code, "Studio rejected the operation; inspect the stable error code");
}
