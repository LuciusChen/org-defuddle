import { readFileSync, readdirSync } from 'node:fs';
import { basename, join } from 'node:path';
import { createRequire } from 'node:module';

const upstreamDir = process.argv[2];
if (!upstreamDir) {
	throw new Error('usage: node scripts/defuddle-reference.mjs <defuddle-checkout>');
}

const require = createRequire(import.meta.url);
const { Defuddle } = require(join(upstreamDir, 'dist', 'node.js'));
const fixturesDir = join(upstreamDir, 'tests', 'fixtures');
const selectedFixture = process.env.ORG_DEFUDDLE_DIFF_FIXTURE;
const fixtureFiles = readdirSync(fixturesDir)
	.filter((file) => file.endsWith('.html'))
	.filter((file) => !selectedFixture || basename(file, '.html') === selectedFixture)
	.sort();
const offlineFetch = () => Promise.reject(new Error('network disabled in parity audit'));
const results = [];

for (const file of fixtureFiles) {
	const name = basename(file, '.html');
	const html = readFileSync(join(fixturesDir, file), 'utf8');
	const frontmatterMatch = html.match(/<!--\s*(\{"url":.*?\})\s*-->/);
	const frontmatter = frontmatterMatch ? JSON.parse(frontmatterMatch[1]) : {};
	const urlName = name.replace(/^[a-z]+--/, '');
	const url = frontmatter.url || `https://${urlName}`;
	const response = await Defuddle(html, url, {
		fetch: offlineFetch,
		separateMarkdown: false,
	});
	results.push({
		name,
		url,
		content: response.content,
	});
}

process.stdout.write(JSON.stringify(results));
