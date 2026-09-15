import { sveltekit } from "@sveltejs/kit/vite";
import adapter from "@sveltejs/adapter-bun";
import { defineConfig } from "vite-plus";
import type { UserConfig } from "vite-plus";

export default defineConfig({
  server: {
    allowedHosts: ["build.dlxstudios.com"],
  },
  plugins: [
    sveltekit({
      adapter: adapter({
        out: "build",
        precompress: true,
        buildOptions: {
          sourcemap: "external",
        },
      }),
    }),
  ],
} as UserConfig);
