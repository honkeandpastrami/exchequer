// SvelteKit writes the hash-routing shell (build/index.html) with root-relative
// asset paths even with `paths.relative`. The app is served under a proxy
// prefix it doesn't know, so make them relative to the page.

import { readFileSync, writeFileSync } from 'node:fs';

const file = new URL('../build/index.html', import.meta.url);
const html = readFileSync(file, 'utf8').replaceAll('"/_app/', '"./_app/');
if (/["'(]\/(?!\/)/.test(html)) {
	console.error('build/index.html still has a root-relative path');
	process.exit(1);
}
writeFileSync(file, html);
