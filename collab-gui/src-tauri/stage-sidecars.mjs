#!/usr/bin/env node
// Build collab + collab-server from the workspace and copy them into
// src-tauri/binaries/<name>-<host-triple>.<ext>, which is where Tauri's
// externalBin machinery expects to find sidecars.
//
// Cross-platform replacement for stage-sidecars.sh — invoked from
// tauri.conf.json's `beforeBuildCommand` / `beforeDevCommand`.

import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, existsSync, chmodSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot  = resolve(scriptDir, '..', '..');
const binDir    = join(scriptDir, 'binaries');

// Determine the host target triple from rustc — same logic as the old script.
let triple;
try {
  const out = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const match = out.match(/^host:\s*(\S+)/m);
  if (!match) throw new Error('no "host:" line in rustc -vV output');
  triple = match[1];
} catch (err) {
  console.error('Could not determine host target triple via `rustc -vV`.');
  console.error('Is the Rust toolchain installed and on PATH? https://rustup.rs/');
  console.error(`  (${err.message})`);
  process.exit(1);
}

const profile   = process.env.PROFILE || 'release';
const cargoArgs = [
  'build',
  ...(profile === 'release' ? ['--release'] : []),
  '--manifest-path', join(repoRoot, 'Cargo.toml'),
  '-p', 'holdmybeer-cli',
  '-p', 'holdmybeer-server',
];

console.log(`Building collab + collab-server (${profile}, ${triple})...`);
const build = spawnSync('cargo', cargoArgs, { stdio: 'inherit' });
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

mkdirSync(binDir, { recursive: true });

const exeSuffix = process.platform === 'win32' ? '.exe' : '';
for (const name of ['collab', 'collab-server']) {
  const src = join(repoRoot, 'target', profile, `${name}${exeSuffix}`);
  const dst = join(binDir, `${name}-${triple}${exeSuffix}`);
  if (!existsSync(src)) {
    console.error(`missing built binary: ${src}`);
    process.exit(1);
  }
  copyFileSync(src, dst);
  if (process.platform !== 'win32') {
    chmodSync(dst, 0o755);
  }
  console.log(`staged ${dst}`);
}
