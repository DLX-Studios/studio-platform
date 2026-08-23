import { defineConfig } from "vite-plus";

export default defineConfig({
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    ignorePatterns: [],
  },
  lint: {
    ignorePatterns: [],
    options: { typeAware: true, typeCheck: true },
  },
  test: {
    include: ["tests/**/*.{test,spec}.{ts,tsx}"],
    passWithNoTests: true,
  },
  run: {
    cache: true,
  },
});
