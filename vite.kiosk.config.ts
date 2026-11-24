import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";

export default defineConfig({
  root: resolve(__dirname, "kiosk"),
  base: "/",
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  build: {
    outDir: resolve(__dirname, "crates/photofinish-core/kiosk-dist"),
    emptyOutDir: false,
  },
});
