import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    // Default "node" environment — current tests are pure functions. Add
    // jsdom as a PINNED devDependency before switching this if DOM tests
    // ever land; an unpinned environment resolves against stray ancestor
    // node_modules and breaks clean checkouts.
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
