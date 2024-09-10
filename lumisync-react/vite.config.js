import process from "node:process";
import { defineConfig, loadEnv } from "vite";
import react from '@vitejs/plugin-react'

/** @type {import('vite').UserConfig} */
export default defineConfig(async ({ command, mode }) => {
    const env = { ...process.env, ...loadEnv(mode, process.cwd()) };

    return {
        plugins: [react()],
        define: {
            __APP_ENV__: JSON.stringify(env.APP_ENV ?? "development"),
            __APP_API_URL__: JSON.stringify(env.APP_API_URL ?? "http://localhost:3000"),
        },
    };
});