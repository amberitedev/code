const fs = require("fs");
const path = require("path");
const cp = require("child_process");

const pnpmStore = "C:/Users/ilai/amberite/node_modules/.pnpm";
const frontendNm = "C:/Users/ilai/amberite/apps/app/frontend/node_modules";

// Read packages to find the right version in the store
function findInStore(pkgName) {
  const dirName = pkgName.replace(/\//g, "+").replace(/@/g, "");
  const items = fs.readdirSync(pnpmStore);
  // Try to find the version starting with the package name
  const match = items.filter(i => i.startsWith(dirName + "@"));
  if (match.length === 0) return null;
  
  // For scoped packages, filter more carefully
  if (pkgName.startsWith("@")) {
    const exact = match.filter(i => {
      // Parse the store name to extract the package part
      // e.g., @scope+name@version
      const pkgPart = i.split("@").slice(0, -1).join("@");
      return pkgPart === dirName;
    });
    return pnpmStore + "/" + (exact[0] || match[0]) + "/node_modules/" + pkgName;
  }
  
  return pnpmStore + "/" + match[0] + "/node_modules/" + pkgName;
}

const copied = new Set();
const errors = [];

function copyPackage(pkgName) {
  if (copied.has(pkgName)) return;
  copied.add(pkgName);

  const dstPath = frontendNm + "/" + pkgName;
  
  // Check if already properly copied (has package.json)
  try {
    if (fs.existsSync(dstPath + "/package.json")) {
      // Verify it has actual content
      const stat = fs.statSync(dstPath + "/package.json");
      if (stat.size > 10) return; // Already properly copied
    }
  } catch (e) {
    // Broken junction - remove it
    try { fs.rmSync(dstPath, { recursive: true, force: true }); } catch (e) {}
  }

  const srcPath = findInStore(pkgName);
  if (!srcPath) {
    errors.push("NOT FOUND: " + pkgName);
    return;
  }

  try {
    const srcStat = fs.statSync(srcPath);
    if (!srcStat.isDirectory()) {
      errors.push("NOT A DIR: " + pkgName + " at " + srcPath);
      return;
    }
  } catch (e) {
    errors.push("CANNOT ACCESS: " + pkgName + " - " + e.message.substring(0, 80));
    return;
  }

  console.log("Copying: " + pkgName);
  
  // Create parent directory
  const parentDir = path.dirname(dstPath);
  fs.mkdirSync(parentDir, { recursive: true });
  
  // Use robocopy to copy the files
  try {
    cp.execSync(
      'robocopy "' + srcPath + '" "' + dstPath + '" /E /R:1 /W:1 /NFL /NDL /NJH /NJS',
      { encoding: "utf8", timeout: 30000, stdio: "pipe" }
    );
  } catch (e) {
    errors.push("ROBOCOPY FAILED: " + pkgName + " - " + (e.message || e.stdout || "").substring(0, 100));
    return;
  }

  // Verify copy succeeded
  if (!fs.existsSync(dstPath + "/package.json")) {
    errors.push("COPY INCOMPLETE: " + pkgName + " - no package.json");
    return;
  }

  // Read package.json to get dependencies and recurse
  try {
    const pkg = JSON.parse(fs.readFileSync(dstPath + "/package.json", "utf8"));
    const deps = { ...(pkg.dependencies || {}), ...(pkg.peerDependencies || {}) };
    for (const dep of Object.keys(deps)) {
      // Skip @types (type info only) and optional peers
      if (dep.startsWith("@types/")) continue;
      copyPackage(dep);
    }
  } catch (e) {
    errors.push("PARSE ERROR: " + pkgName + " - " + e.message.substring(0, 80));
  }
}

// Read frontend package.json to get top-level deps
const frontendPkg = JSON.parse(fs.readFileSync(frontendNm + "/../package.json", "utf8"));
const allDeps = { ...frontendPkg.dependencies, ...frontendPkg.devDependencies };

console.log("Top-level deps to install:");
Object.keys(allDeps).forEach(d => console.log("  " + d + ": " + allDeps[d]));

// Copy each top-level dependency
for (const dep of Object.keys(allDeps)) {
  // Skip workspace deps - find them in the store directly
  if (allDeps[dep].startsWith("workspace:")) continue;
  copyPackage(dep);
}

console.log("\n--- Summary ---");
console.log("Copied: " + [...copied].join(", "));
console.log("Total: " + copied.size + " packages");
if (errors.length > 0) {
  console.log("\nErrors (" + errors.length + "):");
  errors.forEach(e => console.log("  " + e));
}
