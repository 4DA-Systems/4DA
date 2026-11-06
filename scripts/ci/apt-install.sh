#!/usr/bin/env bash
# SPDX-License-Identifier: FSL-1.1-Apache-2.0
# apt-install.sh <package>... — install Linux build dependencies on a GitHub-hosted
# runner with every step BOUNDED, retried, and loud when it gives up.
#
# Why: `apt-get` has no overall timeout. On 2026-11-06 the "Install Linux system
# dependencies" step hung for 20-30 minutes at least five times in one day (PR checks
# and merge-queue runs alike) until the job timeout or a manual cancel, each time
# stalling the merge queue behind it. Two causes look identical from the outside:
# a stalled mirror connection, and the image's own apt/unattended-upgrade holding the
# dpkg lock at boot. This bounds both and retries, so a hang costs minutes, not the job.
#
# Also drops the image's third-party apt sources first: `apt-get update` fails
# WHOLESALE when any single one is unreachable (2026-08-28: packages.microsoft.com
# returned 403 for the `noble` azure-cli repo and killed the job at 14 s). gtk/webkit
# come from Ubuntu's own repos.
set -uo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: apt-install.sh <package>..." >&2
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
export DEBIAN_FRONTEND=noninteractive

for attempt in 1 2 3; do
  if timeout 300 sudo -E apt-get "${APT_OPTS[@]}" update \
     && timeout 600 sudo -E apt-get "${APT_OPTS[@]}" install -y "$@"; then
    echo "apt-install: installed $* (attempt $attempt)"
    exit 0
  fi
  echo "::warning::apt-install: attempt $attempt failed or timed out; retrying in 20 s"
  sleep 20
done

echo "::error::apt-install: gave up after 3 bounded attempts (mirror unreachable or dpkg lock held)"
exit 1
