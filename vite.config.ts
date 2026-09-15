import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

// Tauri 期望固定的 dev 端口，失败时直接报错而不是换端口
const host = process.env.TAURI_DEV_HOST;

const pkg = JSON.parse(
  readFileSync(join(dirname(fileURLToPath(import.meta.url)), "package.json"), "utf8")
) as { version: string };

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    // cargo 的 target/ 在项目根（workspace），里面的 agenthub.exe 运行时被 Windows 锁定，
    // chokidar 试图 watch 它会抛 EBUSY 直接打崩 vite。Rust 侧产物与前端 HMR 无关，全部忽略。
    watch: {
      ignored: ["**/target/**", "**/src-tauri/**", "**/dist/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome110",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
