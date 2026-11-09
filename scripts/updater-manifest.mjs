// Writes the updater manifests (latest.json, latest-proxy.json) next to the release assets.
// release.yml uploads them to the version's own (draft) release; clients read them through
// https://github.com/<repo>/releases/latest/download/..., which only resolves to a release
// once it is published and not a pre-release.
//
// Usage: node scripts/updater-manifest.mjs <assets-dir>
// Env:   GITHUB_REPOSITORY, GITHUB_REF_NAME (the tag), RELEASE_NOTES_FILE (optional)
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, resolve } from "node:path";

// Prefix for latest-proxy.json. The client also rotates through the other mirrors and
// GitHub itself if this one fails (src-tauri/src/updater/download.rs).
const MIRROR = "https://gh-proxy.org/";

export function buildManifest({ tag, notes, publishedAt, setupUrl, signature }, mirrorPrefix = "") {
  const platform = { url: mirrorPrefix + setupUrl, signature };
  return {
    version: tag.replace(/^v/, ""),
    notes,
    pub_date: publishedAt,
    platforms: {
      "windows-x86_64-nsis": { ...platform },
      "windows-x86_64": { ...platform },
    },
  };
}

/** Picks the updater installer and its exact .sig from asset names. */
export function pickAssets(tag, names) {
  const setup = names.find((n) => n.endsWith("_x64-setup.exe"));
  if (!setup) throw new Error(`${tag} 的发布文件中找不到 _x64-setup.exe 安装包`);
  const sig = names.find((n) => n === `${setup}.sig`);
  if (!sig) throw new Error(`${tag} 的发布文件中找不到 ${setup}.sig 签名文件`);
  return { setup, sig };
}

function main() {
  const dir = process.argv[2];
  const repo = process.env.GITHUB_REPOSITORY;
  const tag = process.env.GITHUB_REF_NAME;
  if (!dir || !repo || !tag) throw new Error("用法：node scripts/updater-manifest.mjs <assets-dir>，并设置 GITHUB_REPOSITORY 和 GITHUB_REF_NAME");

  const { setup, sig } = pickAssets(tag, readdirSync(dir));
  const notesFile = process.env.RELEASE_NOTES_FILE;
  const input = {
    tag,
    notes: notesFile ? readFileSync(notesFile, "utf8").trim() : "",
    publishedAt: new Date().toISOString(),
    setupUrl: `https://github.com/${repo}/releases/download/${tag}/${setup}`,
    signature: readFileSync(join(dir, sig), "utf8").trim(),
  };
  writeFileSync(join(dir, "latest.json"), JSON.stringify(buildManifest(input), null, 2));
  writeFileSync(join(dir, "latest-proxy.json"), JSON.stringify(buildManifest(input, MIRROR), null, 2));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  try { main(); } catch (e) { console.error(e.message); process.exit(1); }
}
