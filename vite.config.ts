import { cpSync, createReadStream, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { defineConfig, type PluginOption } from "vite";
import packageJson from "./package.json";
import react from "@vitejs/plugin-react";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
const require = createRequire(import.meta.url);
const rootDir = dirname(fileURLToPath(import.meta.url));

function materialIconsPlugin(): PluginOption {
  const getIconSourceDir = () => {
    const entryPath = require.resolve("vscode-material-icons");
    return resolve(dirname(entryPath), "../generated/icons");
  };

  return {
    name: "copy-vscode-material-icons",
    configureServer(server) {
      server.middlewares.use("/assets/material-icons", (req, res, next) => {
        const urlPath = decodeURIComponent((req.url ?? "").split("?")[0] ?? "").replace(/^\/+/, "");
        const iconSourceDir = getIconSourceDir();
        const iconPath = resolve(iconSourceDir, urlPath);

        if (!urlPath.endsWith(".svg") || !iconPath.startsWith(iconSourceDir) || !existsSync(iconPath)) {
          next();
          return;
        }

        res.setHeader("Content-Type", "image/svg+xml");
        createReadStream(iconPath).pipe(res);
      });
    },
    closeBundle() {
      const iconSourceDir = getIconSourceDir();
      const iconOutputDir = resolve(rootDir, "dist/assets/material-icons");

      if (existsSync(iconSourceDir)) {
        cpSync(iconSourceDir, iconOutputDir, { recursive: true });
      }
    },
  };
}

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true
    }),
    react(),
    materialIconsPlugin()
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  define: {
    "import.meta.env.PACKAGE_VERSION": JSON.stringify(packageJson.version),
  },
  resolve: {
    alias: {
      "@": "/src",
    },
  },
  build: {
    chunkSizeWarningLimit: 1024,
  },
}));
