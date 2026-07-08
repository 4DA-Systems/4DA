// Single source of truth for the current release + direct download URLs.
// UPDATE THIS ONE FILE per release: bump `version`/`tag`, and the asset names
// if Tauri's naming changes. Direct links let /download hand users the actual
// installer in one click, bypassing the GitHub release page entirely.
const ORG = "4DA-Systems";
const REPO = "4DA";
const VERSION = "1.0.0";
const TAG = "v1.0.0";
const base = `https://github.com/${ORG}/${REPO}/releases/download/${TAG}`;

export default {
  org: ORG,
  repo: REPO,
  version: VERSION,
  tag: TAG,
  repoUrl: `https://github.com/${ORG}/${REPO}`,
  releaseUrl: `https://github.com/${ORG}/${REPO}/releases/tag/${TAG}`,
  // Exact asset names verified against the v1.0.0 release.
  files: {
    win:      { url: `${base}/4DA.Home_${VERSION}_x64-setup.exe`, ext: ".exe", size: "~30 MB" },
    macArm:   { url: `${base}/4DA.Home_${VERSION}_aarch64.dmg`,   ext: ".dmg", size: "~31 MB" },
    macIntel: { url: `${base}/4DA.Home_${VERSION}_x64.dmg`,       ext: ".dmg", size: "~33 MB" },
    linuxAppImage: { url: `${base}/4DA.Home_${VERSION}_amd64.AppImage`, ext: ".AppImage", size: "~112 MB" },
    linuxDeb: { url: `${base}/4DA.Home_${VERSION}_amd64.deb`,     ext: ".deb", size: "~34 MB" },
    linuxRpm: { url: `${base}/4DA.Home-${VERSION}-1.x86_64.rpm`,  ext: ".rpm", size: "~34 MB" },
  },
};
