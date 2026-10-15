import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const input = process.argv[2] ?? '';
const version = input.startsWith('v') ? input.slice(1) : input;
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('Usage: npm run release -- 1.2.3');
  process.exit(1);
}

const run = (command, args) =>
  execFileSync(command, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();

function compareVersions(left, right) {
  const a = left.split('.').map(Number);
  const b = right.split('.').map(Number);
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

const branch = run('git', ['branch', '--show-current']);
if (!branch) {
  throw new Error('Release must run from a branch, not a detached HEAD.');
}
if (run('git', ['status', '--porcelain'])) {
  throw new Error('Commit or stash all changes before creating a release.');
}
if (!run('git', ['remote', 'get-url', 'origin'])) {
  throw new Error("Git remote 'origin' is not configured.");
}
if (run('git', ['tag', '--list', `v${version}`])) {
  throw new Error(`Tag v${version} already exists.`);
}

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const initialCargo = readFileSync('src-tauri/Cargo.toml', 'utf8');
const cargoVersion = initialCargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const currentVersions = [packageJson.version, tauri.version, cargoVersion].filter(Boolean);
const latestCurrentVersion = currentVersions.reduce((latest, current) =>
  compareVersions(current, latest) > 0 ? current : latest,
);
if (compareVersions(version, latestCurrentVersion) <= 0) {
  throw new Error(`Release version must be newer than ${latestCurrentVersion}.`);
}

packageJson.version = version;
writeFileSync('package.json', `${JSON.stringify(packageJson, null, 2)}\n`);

const packageLock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
packageLock.version = version;
if (packageLock.packages?.['']) packageLock.packages[''].version = version;
writeFileSync('package-lock.json', `${JSON.stringify(packageLock, null, 2)}\n`);

tauri.version = version;
writeFileSync('src-tauri/tauri.conf.json', `${JSON.stringify(tauri, null, 2)}\n`);

const cargoPath = 'src-tauri/Cargo.toml';
const cargoManifest = readFileSync(cargoPath, 'utf8');
const updatedCargo = cargoManifest.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`);
if (updatedCargo === cargoManifest) {
  throw new Error('Could not find the package version in src-tauri/Cargo.toml.');
}
writeFileSync(cargoPath, updatedCargo);

const cargoLockPath = 'src-tauri/Cargo.lock';
const cargoLock = readFileSync(cargoLockPath, 'utf8');
const updatedCargoLock = cargoLock.replace(
  /(\[\[package\]\]\nname = "db-sync-ssh"\nversion = ")[^"]+(")/,
  `$1${version}$2`,
);
if (updatedCargoLock === cargoLock) {
  throw new Error('Could not find db-sync-ssh in src-tauri/Cargo.lock.');
}
writeFileSync(cargoLockPath, updatedCargoLock);

execFileSync('cargo', ['check', '--manifest-path', 'src-tauri/Cargo.toml', '--locked'], {
  stdio: 'inherit',
});

execFileSync(
  'git',
  [
    'add',
    'package.json',
    'package-lock.json',
    'src-tauri/tauri.conf.json',
    'src-tauri/Cargo.toml',
    'src-tauri/Cargo.lock',
  ],
  { stdio: 'inherit' },
);
execFileSync(
  'git',
  [
    'commit',
    '-m',
    `Release v${version}`,
    '-m',
    'Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>',
  ],
  { stdio: 'inherit' },
);
execFileSync('git', ['tag', '-a', `v${version}`, '-m', `Release v${version}`], {
  stdio: 'inherit',
});
execFileSync('git', ['push', 'origin', branch], { stdio: 'inherit' });
execFileSync('git', ['push', 'origin', `v${version}`], { stdio: 'inherit' });

console.log(`Release v${version} pushed. GitHub Actions will build and publish the desktop installers.`);
