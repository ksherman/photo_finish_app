import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";

export default defineConfig({
  root: resolve(import.meta.dirname, "kiosk"),
  base: "/",
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "src"),
    },
  },
  build: {
    outDir: resolve(import.meta.dirname, "crates/photofinish-core/kiosk-dist"),
    emptyOutDir: false,
  },
});
