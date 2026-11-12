// Single source of truth for the current release + download URLs.
// The /download page and the vanity redirect pages (/download/<slug>/) are both
// generated from `files` below, so clean URLs like 4da.ai/download/win never
// change and the buttons never expose the raw versioned GitHub asset URL.
//
// RELEASE SWITCH. `NEXT` is the version the site's screenshots and copy
// describe; `PUBLISHED` says whether its GitHub release is out. While it is
// false, every download link stays on `PREVIOUS` (a release that exists) and
// `/` and `/download` say, in one line, that the download is older than the
// screenshots. After the release is published and its assets are checked
// (gh release view vX.Y.Z --repo 4DA-Systems/4DA --json assets), flip
// PUBLISHED to true, fill in NEXT_SIZES from that release, rebuild and deploy.
// Flipping it before the release exists would point every button at a 404.
const ORG = "4DA-Systems";
const REPO = "4DA";

const PREVIOUS = "1.0.3";
const NEXT = "1.0.4";
const PUBLISHED = false;

// Installer sizes as shown on the buttons. v1.0.3: measured from its release.
// v1.0.4 no longer bundles the 261 MB embedding model (downloaded on first use);
// fill these in from the published assets when flipping PUBLISHED. An empty
// string shows no size rather than a guessed one.
const PREVIOUS_SIZES = { win: "~280 MB", macArm: "~296 MB", macIntel: "~299 MB", linuxAppImage: "~369 MB", linuxDeb: "~299 MB", linuxRpm: "~299 MB" };
const NEXT_SIZES = { win: "", macArm: "", macIntel: "", linuxAppImage: "", linuxDeb: "", linuxRpm: "" };

const VERSION = PUBLISHED ? NEXT : PREVIOUS;
const SIZES = PUBLISHED ? NEXT_SIZES : PREVIOUS_SIZES;
const TAG = `v${VERSION}`;
const base = `https://github.com/${ORG}/${REPO}/releases/download/${TAG}`;

// Tauri's asset naming, verified against the v1.0.3 release
// (gh release view v1.0.3 --repo 4DA-Systems/4DA --json assets).
const files = {
  win:           { slug: "win",       os: "Windows",               url: `${base}/4DA_${VERSION}_x64-setup.exe`,    ext: ".exe",      size: SIZES.win },
  macArm:        { slug: "mac-arm",   os: "macOS (Apple Silicon)", url: `${base}/4DA_${VERSION}_aarch64.dmg`,      ext: ".dmg",      size: SIZES.macArm },
  macIntel:      { slug: "mac-intel", os: "macOS (Intel)",         url: `${base}/4DA_${VERSION}_x64.dmg`,          ext: ".dmg",      size: SIZES.macIntel },
  linuxAppImage: { slug: "linux",     os: "Linux (AppImage)",      url: `${base}/4DA_${VERSION}_amd64.AppImage`,   ext: ".AppImage", size: SIZES.linuxAppImage },
  linuxDeb:      { slug: "linux-deb", os: "Linux (.deb)",          url: `${base}/4DA_${VERSION}_amd64.deb`,        ext: ".deb",      size: SIZES.linuxDeb },
  linuxRpm:      { slug: "linux-rpm", os: "Linux (.rpm)",          url: `${base}/4DA-${VERSION}-1.x86_64.rpm`,     ext: ".rpm",      size: SIZES.linuxRpm },
};

export default {
  org: ORG,
  repo: REPO,
  version: VERSION,
  tag: TAG,
  // The version the screenshots and copy describe, and whether it can be downloaded yet.
  next: NEXT,
  published: PUBLISHED,
  repoUrl: `https://github.com/${ORG}/${REPO}`,
  releaseUrl: `https://github.com/${ORG}/${REPO}/releases/tag/${TAG}`,
  files,
  list: Object.values(files), // drives the vanity redirect pages
};
