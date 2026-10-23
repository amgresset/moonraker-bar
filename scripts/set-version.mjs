#!/usr/bin/env node
// Sets the app version everywhere it lives: package.json, src-tauri/tauri.conf.json,
// src-tauri/Cargo.toml and src-tauri/Cargo.lock.
//
//   npm run version:set 0.2.0

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error("usage: npm run version:set <x.y.z>");
  process.exit(1);
}

const edit = (path, fn) => {
  const before = readFileSync(path, "utf8");
  const after = fn(before);
  if (after === before) throw new Error(`${path}: version field not found`);
  writeFileSync(path, after);
  console.log(`  ${path}`);
};

edit("package.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
edit("src-tauri/tauri.conf.json", (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
edit("src-tauri/Cargo.toml", (s) => s.replace(/(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/, `$1${version}$2`));
edit("src-tauri/Cargo.lock", (s) => s.replace(/(name = "moonraker-bar"\nversion = ")[^"]+(")/, `$1${version}$2`));

console.log(`\nVersion set to ${version}. Next:\n  git commit -am "Release v${version}" && git tag v${version} && git push && git push --tags`);
