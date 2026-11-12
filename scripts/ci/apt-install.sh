#!/usr/bin/env bash
# SPDX-License-Identifier: FSL-1.1-Apache-2.0
# apt-install.sh <package>... — install Linux build dependencies on a GitHub-hosted
# runner with every step BOUNDED, retried, and loud when it gives up.
#
# Why: on 2026-11-06 the "Install Linux system dependencies" step ran 20-30 minutes at
# least five times (PR checks and merge-queue runs alike), each time stalling the merge
# queue. The job log showed what it was: not a hang, but azure.archive.ubuntu.com
# serving ~10-40 kB/s while `libwebkit2gtk-4.1-dev` dragged in its Recommends (the
# gstreamer plugin set, tens of MB). So this script:
#   1. bounds every apt call (`timeout`, Acquire timeouts, DPkg::Lock::Timeout);
#   2. retries, and from the second attempt points the runner's mirror list at
#      archive.ubuntu.com, so a crawling Azure mirror is not retried as-is (packages
#      already in /var/cache/apt/archives are kept between attempts);
#   3. with APT_NO_RECOMMENDS=1 skips Recommends. Compile/test jobs need only the -dev
#      headers; the release workflow leaves it unset (bundling may want media plugins).
#
# Also drops the image's third-party apt sources first: `apt-get update` fails
# WHOLESALE when any single one is unreachable (2026-08-28: packages.microsoft.com
# returned 403 for the `noble` azure-cli repo and killed the job at 14 s).
set -uo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: [APT_NO_RECOMMENDS=1] apt-install.sh <package>..." >&2
  exit 2
fi

sudo rm -f /etc/apt/sources.list.d/microsoft-prod.list \
           /etc/apt/sources.list.d/microsoft-prod.sources \
           /etc/apt/sources.list.d/azure-cli.list \
           /etc/apt/sources.list.d/azure-cli.sources

APT_OPTS=(
  -o Acquire::Retries=3
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
  -o DPkg::Lock::Timeout=180
)
INSTALL_OPTS=(-y)
if [ "${APT_NO_RECOMMENDS:-0}" = "1" ]; then
  INSTALL_OPTS+=(--no-install-recommends)
fi
export DEBIAN_FRONTEND=noninteractive

use_fallback_mirror() {
  # Off the runner's Azure default, onto the first mirror this runner resolves
  # (apt-mirror-select.sh): archive.ubuntu.com, else mirrors.edge.kernel.org.
  bash "$(dirname "$0")/apt-mirror-select.sh" --avoid-default
}

# Before the first attempt: a mirror that does not even resolve is not tried at all.
bash "$(dirname "$0")/apt-mirror-select.sh"

for attempt in 1 2 3; do
  if [ "$attempt" -ge 2 ]; then
    use_fallback_mirror
  fi
  if timeout 240 sudo -E apt-get "${APT_OPTS[@]}" update \
     && timeout 600 sudo -E apt-get "${APT_OPTS[@]}" install "${INSTALL_OPTS[@]}" "$@"; then
    echo "apt-install: installed $* (attempt $attempt)"
    exit 0
  fi
  echo "::warning::apt-install: attempt $attempt failed or timed out; retrying in 15 s"
  sleep 15
done

echo "::error::apt-install: gave up after 3 bounded attempts (mirror too slow or unreachable, or dpkg lock held)"
exit 1
