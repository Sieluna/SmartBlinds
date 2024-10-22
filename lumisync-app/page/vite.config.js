import process from 'node:process';
import { defineConfig, loadEnv } from 'vite';
import solidPlugin from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';

/** @type {import('vite').UserConfig} */
export default defineConfig(async ({ mode }) => {
  const env = { ...process.env, ...loadEnv(mode, process.cwd()) };

  return {
    plugins: [
      tailwindcss(),
      solidPlugin(),
    ],
    clearScreen: false,
    define: {
      __APP_ENV__: JSON.stringify(env.APP_ENV ?? 'development'),
      __APP_API_URL__: JSON.stringify(env.APP_API_URL ?? 'http://localhost:3000'),
    },
    server: {
      port: 1420,
      strictPort: true,
      host: env.TAURI_DEV_HOST ?? false,
      hmr: env.TAURI_DEV_HOST
        ? {
            protocol: 'ws',
            host: env.TAURI_DEV_HOST,
            port: 1421,
          }
        : undefined,
    },
  };
});
