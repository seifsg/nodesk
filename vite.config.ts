// Landing page build: sources in site/, static output in dist/ (index.html at its root).
import { defineConfig } from "vite";

export default defineConfig({
  root: "site",
  build: { outDir: "../dist", emptyOutDir: true },
});
