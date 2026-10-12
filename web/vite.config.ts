import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter(),
			// Served by the API container under whatever prefix the proxy gives
			// it: routes live in the hash and every path is relative.
			router: { type: 'hash' },
			paths: { relative: true }
		})
	],
	server: {
		// `npm run dev`: the API from a local server (`exchequer-server serve`).
		proxy: { '/api': process.env.EXCHEQUER_API ?? 'http://127.0.0.1:7438' }
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
