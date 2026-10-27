import { cpSync, createReadStream, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { defineConfig, type PluginOption } from "vite";
import packageJson from "./package.json" with { type: "json" };
import react from "@vitejs/plugin-react";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
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

/**
 * xterm's WebGL renderer paints a background rectangle for every cell whose
 * background word is non-zero, but dim, italic and extended attributes are
 * stored in that word too. A dim cell on the default background therefore gets
 * the theme background painted opaque behind it, a dark box once the window
 * blur makes the terminal background transparent. Still so in the 0.20 betas;
 * this treats any default-colored background as none, and fails the build when
 * an upgraded addon no longer has the code it rewrites.
 */
function xtermWebglDefaultBackgroundPlugin(): PluginOption {
  // `f` is the cell's background word, read through the renderer's BG offset.
  const target = "f=t.cells[h+ze],";
  const CM_MASK = 0x3000000;
  return {
    name: "xterm-webgl-default-background",
    transform(code, id) {
      if (!/[\\/]@xterm[\\/]addon-webgl[\\/]lib[\\/]addon-webgl\.mjs(\?|$)/.test(id)) return null;
      if (code.split(target).length !== 2) {
        throw new Error(
          "@xterm/addon-webgl changed: re-check the default-background fix in vite.config.ts",
        );
      }
      return { code: code.replace(target, `${target}f&${CM_MASK}||(f=0),`), map: null };
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
    materialIconsPlugin(),
    xtermWebglDefaultBackgroundPlugin()
  ],
  // Pre-bundled dependencies skip plugin transforms in dev.
  optimizeDeps: {
    exclude: ["@xterm/addon-webgl"],
  },

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
    rolldownOptions: {
      checks: {
        pluginTimings: false,
      },
    },
    chunkSizeWarningLimit: 2048,
  },
}));
