#!/usr/bin/env node
/**
 * CLI `vibegame` — ponto de entrada estável para o instalador unificado do monorepo.
 * Usa apenas Node (stdlib); `bun install` / `bun run build` são responsabilidade do instalador.
 */
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const createScript = join(root, 'create-vibegame', 'index.js');

function help() {
  console.log(`vibegame ${pkg.version} — VibeGame (GameDev monorepo)`);
  console.log('');
  console.log('  vibegame create <name>   Create a new project (scaffold)');
  console.log('  vibegame --version       Show version');
  console.log('  vibegame help            This message');
  console.log('');
  console.log(`Library root: ${root}`);
}

const argv = process.argv.slice(2);
const first = argv[0];

if (first === 'create' || first === '-c') {
  const rest = argv.slice(1);
  const r = spawnSync(process.execPath, [createScript, ...rest], {
    stdio: 'inherit',
    cwd: process.cwd(),
  });
  process.exit(r.status ?? 1);
}

if (first === '--version' || first === '-v') {
  console.log(pkg.version);
  process.exit(0);
}

if (
  first === 'help' ||
  first === '--help' ||
  first === '-h' ||
  first === undefined
) {
  help();
  process.exit(first === undefined ? 0 : 0);
}

console.error(`Unknown command: ${first}`);
console.error('Try: vibegame help');
process.exit(1);
