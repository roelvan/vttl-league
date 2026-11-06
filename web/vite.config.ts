import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: { '/api': { target: 'http://127.0.0.1:3100', ws: true } },
  },
  build: {
    target: 'es2022',
    cssCodeSplit: false,
  },
});
