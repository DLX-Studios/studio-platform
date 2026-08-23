import { beginBatch, endBatch } from "./scheduler";

/** Group state writes and flush dependent effects once at the outer boundary. */
export function batch<T>(run: () => T): T {
  beginBatch();
  const result = run();
  endBatch();
  return result;
}
