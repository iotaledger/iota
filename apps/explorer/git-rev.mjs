import { execSync } from 'child_process';
import fs from 'fs';

// Get the current git commit hash
const commitHash = execSync('git rev-parse HEAD').toString().trim();
console.log(`Current commit hash: ${commitHash}`);

// Save the commit hash to a file
const filePath = './rev.json'
fs.writeFileSync(filePath, JSON.stringify({
  rev: commitHash
}));

console.log(`Commit hash saved to ${filePath}`);

