// 设计令牌守卫：tokens.css 之外不允许出现具体色值。
// 用法：node scripts/check-tokens.mjs
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.cwd();
const srcDir = join(root, "src");
const allowedFile = join(srcDir, "styles", "tokens.css");
const colorPattern = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/;
const skipPattern = /token-fallback|check-tokens-ignore/;

function walk(dir) {
  const entries = [];
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const stat = statSync(full);
    if (stat.isDirectory()) entries.push(...walk(full));
    else if (/\.(css|ts|tsx|html)$/.test(name)) entries.push(full);
  }
  return entries;
}

let failures = 0;
for (const file of walk(srcDir)) {
  if (file === allowedFile) continue;
  const lines = readFileSync(file, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    if (skipPattern.test(line)) return;
    if (colorPattern.test(line)) {
      failures += 1;
      console.error(`${relative(root, file)}:${index + 1}: 发现令牌外色值 -> ${line.trim()}`);
    }
  });
}

if (failures > 0) {
  console.error(`\n共 ${failures} 处令牌外色值，请改用 --md-sys-color-* 令牌。`);
  process.exit(1);
}
console.log("颜色令牌检查通过：tokens.css 之外没有色值。");
