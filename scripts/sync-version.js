import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');

const cargoToml = readFileSync(join(root, 'src-tauri', 'Cargo.toml'), 'utf-8');
const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
  console.error('Could not find version in Cargo.toml');
  process.exit(1);
}

const version = match[1];
const pkgPath = join(root, 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf-8'));

if (pkg.version !== version) {
  pkg.version = version;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf-8');
  console.log(`Synced package.json version to ${version}`);
} else {
  console.log(`package.json version already ${version}, no update needed`);
}
