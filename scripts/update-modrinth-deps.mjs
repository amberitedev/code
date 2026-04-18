#!/usr/bin/env node
/**
 * Update Modrinth Dependencies
 * 
 * This script reads the modrinthVersion from root package.json
 * and updates all package.json files to use that version.
 * 
 * Usage: node scripts/update-modrinth-deps.js
 */

import { readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const rootDir = join(__dirname, '..');

// Read root package.json
const rootPackageJson = JSON.parse(
  readFileSync(join(rootDir, 'package.json'), 'utf-8')
);

const modrinthVersion = rootPackageJson.config?.modrinthVersion;

if (!modrinthVersion) {
  console.error('❌ No config.modrinthVersion found in root package.json');
  process.exit(1);
}

console.log(`📦 Updating Modrinth dependencies to ${modrinthVersion}...`);

const modrinthPackages = [
  '@modrinth/ui',
  '@modrinth/api-client',
  '@modrinth/assets',
  '@modrinth/utils',
  '@modrinth/blog',
  '@modrinth/tooling-config',
];

const modrinthDep = `github:modrinth/code#${modrinthVersion}`;

// Files to update
const filesToUpdate = [
  'apps/app/frontend/package.json',
  'packages/config/package.json',
];

let updatedCount = 0;

for (const file of filesToUpdate) {
  const filePath = join(rootDir, file);
  
  try {
    const content = readFileSync(filePath, 'utf-8');
    const pkg = JSON.parse(content);
    let modified = false;
    
    // Update dependencies
    if (pkg.dependencies) {
      for (const dep of modrinthPackages) {
        if (pkg.dependencies[dep]) {
          pkg.dependencies[dep] = modrinthDep;
          modified = true;
        }
      }
    }
    
    // Update devDependencies
    if (pkg.devDependencies) {
      for (const dep of modrinthPackages) {
        if (pkg.devDependencies[dep]) {
          pkg.devDependencies[dep] = modrinthDep;
          modified = true;
        }
      }
    }
    
    if (modified) {
      writeFileSync(filePath, JSON.stringify(pkg, null, '\t') + '\n');
      console.log(`  ✅ Updated ${file}`);
      updatedCount++;
    } else {
      console.log(`  ⏭️  Skipped ${file} (no Modrinth deps)`);
    }
  } catch (err) {
    console.error(`  ❌ Error updating ${file}: ${err.message}`);
  }
}

console.log(`\n🎉 Done! Updated ${updatedCount} files.`);
console.log(`📝 Run "pnpm install" to apply the changes.`);
