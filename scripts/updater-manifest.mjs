import { writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const MIRROR = "https://update.hwdns.net/";
const UPDATER_TAG = "updater";

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

async function api(path, token, init = {}) {
  return fetch(`https://api.github.com${path}`, {
    ...init,
    headers: { Authorization: `Bearer ${token}`, Accept: "application/vnd.github+json", ...init.headers },
  });
}

async function main() {
  const token = process.env.GITHUB_TOKEN;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!token || !repo) throw new Error("缺少环境变量 GITHUB_TOKEN 或 GITHUB_REPOSITORY");

  const res = await api(`/repos/${repo}/releases/latest`, token);
  if (!res.ok) throw new Error(`获取最新 Release 失败：HTTP ${res.status}`);
  const release = await res.json();
  const setup = release.assets.find((a) => a.name.endsWith("_x64-setup.exe"));
  if (!setup) throw new Error(`Release ${release.tag_name} 中找不到 _x64-setup.exe 安装包`);
  const sig = release.assets.find((a) => a.name === `${setup.name}.sig`);
  if (!sig) throw new Error(`Release ${release.tag_name} 中找不到 ${setup.name}.sig 签名文件`);

  const sigRes = await fetch(sig.browser_download_url);
  if (!sigRes.ok) throw new Error(`下载签名文件失败：HTTP ${sigRes.status}`);
  const signature = (await sigRes.text()).trim();

  const input = {
    tag: release.tag_name,
    notes: release.body ?? "",
    publishedAt: release.published_at,
    setupUrl: setup.browser_download_url,
    signature,
  };
  writeFileSync("latest.json", JSON.stringify(buildManifest(input), null, 2));
  writeFileSync("latest-proxy.json", JSON.stringify(buildManifest(input, MIRROR), null, 2));

  const env = { ...process.env, GH_TOKEN: token };
  const exists = await api(`/repos/${repo}/releases/tags/${UPDATER_TAG}`, token);
  if (exists.status === 404) {
    // Non-draft prerelease so it never becomes "latest".
    execFileSync("gh", ["release", "create", UPDATER_TAG, "--repo", repo, "--title", UPDATER_TAG,
      "--notes", "Auto-update manifests (maintained by CI).", "--prerelease"], { stdio: "inherit", env });
  } else if (!exists.ok) {
    throw new Error(`查询 ${UPDATER_TAG} Release 失败：HTTP ${exists.status}`);
  }
  execFileSync("gh", ["release", "upload", UPDATER_TAG, "latest.json", "latest-proxy.json", "--clobber", "--repo", repo],
    { stdio: "inherit", env });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  main().catch((e) => { console.error(e.message); process.exit(1); });
}
