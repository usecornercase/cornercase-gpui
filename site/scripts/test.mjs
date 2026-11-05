import { execFileSync } from 'node:child_process';
import { readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { build } from 'esbuild';

const out = join(tmpdir(), `cornercase-gpui-site-tests-${process.pid}`);
const files = readdirSync('tests')
  .filter((name) => name.endsWith('.test.ts'))
  .map((name) => join('tests', name));

await build({ entryPoints: files, bundle: true, platform: 'node', format: 'esm', outdir: out, logLevel: 'warning', outExtension: { '.js': '.mjs' } });

try {
  const bundled = files.map((file) => join(out, file.replace(/^tests[/\\]/, '').replace(/\.ts$/, '.mjs')));
  execFileSync(process.execPath, ['--test', ...bundled], { stdio: 'inherit' });
} finally {
  rmSync(out, { recursive: true, force: true });
}
