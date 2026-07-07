import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	// Tauri expects a fixed dev port (see src-tauri/tauri.conf.json devUrl)
	clearScreen: false,
	server: { port: 1420, strictPort: true }
});
