/**
 * OpenNeoveg — Zero-Dependency TypeScript Bundler (Node 24+ native `stripTypeScriptTypes`)
 * Bundles `src/types.ts`, `src/ui/icons.ts`, `src/ui/faceThumbnails.ts`,
 * `src/engine/compositor.ts`, `src/engine/audioEngine.ts`,
 * `src/ui/timeline.ts`, `src/ui/dockManager.ts`, and `src/main.ts` into `public/bundle.js`.
 */

import fs from 'node:fs';
import path from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const modulesOrder = [
  'src/types.ts',
  'src/ui/icons.ts',
  'src/ui/faceThumbnails.ts',
  'src/engine/compositor.ts',
  'src/engine/audioEngine.ts',
  'src/ui/timeline.ts',
  'src/ui/dockManager.ts',
  'src/main.ts',
];

let combined = `// OpenNeoveg 21 — Compiled TypeScript Studio Bundle\n// Generated from frontend/src/*.ts\n\n`;

for (const relPath of modulesOrder) {
  const fullPath = path.join(__dirname, relPath);
  const rawTs = fs.readFileSync(fullPath, 'utf8');
  const strippedJs = stripTypeScriptTypes(rawTs, { mode: 'strip', sourceUrl: relPath });

  const cleaned = strippedJs
    .replace(/^\s*import\s+[\s\S]*?from\s+['"][^'"]+['"];?\s*$/gm, '')
    .replace(/^\s*export\s+(class|function|const|let|var)/gm, '$1')
    .replace(/^\s*export\s+\{[\s\S]*?\};?\s*$/gm, '');

  combined += `\n// ─── MODULE: ${relPath} ───\n` + cleaned + '\n';
}

const outPath = path.join(__dirname, 'public', 'bundle.js');
fs.writeFileSync(outPath, combined, 'utf8');
console.log(`[Neoveg TypeScript Build] Successfully compiled ${modulesOrder.length} TypeScript modules -> ${outPath} (${combined.length} bytes)`);
