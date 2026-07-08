// Single source of truth for the current release + download URLs.
// UPDATE THIS ONE FILE per release: bump `VERSION`/`TAG` (and asset names if
// Tauri's naming changes). The /download page and the vanity redirect pages
// (/download/<slug>/) are both generated from `files` below, so clean URLs
// like 4da.ai/download/win never change and the buttons never expose the raw
// versioned GitHub asset URL.
const ORG = "4DA-Systems";
const REPO = "4DA";
const VERSION = "1.0.0";
const TAG = "v1.0.0";
const base = `https://github.com/${ORG}/${REPO}/releases/download/${TAG}`;

// Exact asset names verified against the v1.0.0 release
// (gh release view v1.0.0 --repo 4DA-Systems/4DA --json assets).
const files = {
  win:           { slug: "win",       os: "Windows",               url: `${base}/4DA.Home_${VERSION}_x64-setup.exe`, ext: ".exe",      size: "~30 MB" },
  macArm:        { slug: "mac-arm",   os: "macOS (Apple Silicon)", url: `${base}/4DA.Home_${VERSION}_aarch64.dmg`,   ext: ".dmg",      size: "~31 MB" },
  macIntel:      { slug: "mac-intel", os: "macOS (Intel)",         url: `${base}/4DA.Home_${VERSION}_x64.dmg`,       ext: ".dmg",      size: "~33 MB" },
  linuxAppImage: { slug: "linux",     os: "Linux (AppImage)",      url: `${base}/4DA.Home_${VERSION}_amd64.AppImage`, ext: ".AppImage", size: "~112 MB" },
  linuxDeb:      { slug: "linux-deb", os: "Linux (.deb)",          url: `${base}/4DA.Home_${VERSION}_amd64.deb`,     ext: ".deb",      size: "~34 MB" },
  linuxRpm:      { slug: "linux-rpm", os: "Linux (.rpm)",          url: `${base}/4DA.Home-${VERSION}-1.x86_64.rpm`,  ext: ".rpm",      size: "~34 MB" },
};

export default {
  org: ORG,
  repo: REPO,
  version: VERSION,
  tag: TAG,
  repoUrl: `https://github.com/${ORG}/${REPO}`,
  releaseUrl: `https://github.com/${ORG}/${REPO}/releases/tag/${TAG}`,
  files,
  list: Object.values(files), // drives the vanity redirect pages
};
