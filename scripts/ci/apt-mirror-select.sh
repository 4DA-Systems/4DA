#!/usr/bin/env bash
# SPDX-License-Identifier: FSL-1.1-Apache-2.0
# apt-mirror-select.sh [--avoid-default] — point apt at an Ubuntu mirror this runner can
# actually RESOLVE, before anything runs `apt-get update`.
#
# Why: from about 14:00 UTC on 2026-11-11 GitHub-hosted ubuntu-26.04 runners could not
# resolve azure.archive.ubuntu.com, archive.ubuntu.com or security.ubuntu.com ("Temporary
# failure resolving"), while every other host resolved (the npm-based jobs passed). Every
# Rust and E2E job died installing system packages, main's scheduled Hermetic run
# included, for hours. apt-install.sh's fallback only swapped one ubuntu.com host for
# another, and `apt-get update` fails wholesale when ANY source (security.ubuntu.com)
# cannot be reached. mirrors.edge.kernel.org carries the full archive, security pocket
# included.
#
# Without --avoid-default: if every Ubuntu host in the sources resolves, nothing changes.
# With --avoid-default: the runner's Azure default is not used even when it resolves (a
# mirror that resolves but crawls, apt-install.sh's case of 2026-11-06).
# Otherwise every archive and security source is rewritten to the first candidate that
# resolves. Never fails the job: when no candidate resolves it warns and leaves apt as is.
set -uo pipefail

resolves() { getent hosts "$1" >/dev/null 2>&1; }

avoid_default=0
[ "${1:-}" = "--avoid-default" ] && avoid_default=1

if [ "$avoid_default" = "0" ] && resolves azure.archive.ubuntu.com && resolves archive.ubuntu.com && resolves security.ubuntu.com; then
  echo "apt-mirror-select: the runner's Ubuntu mirrors resolve; sources unchanged"
  exit 0
fi

base=""
for candidate in http://archive.ubuntu.com/ubuntu http://mirrors.edge.kernel.org/ubuntu; do
  host="${candidate#http://}"
  host="${host%%/*}"
  if resolves "$host"; then
    # archive.ubuntu.com is only a fix when security.ubuntu.com resolves too, or when it
    # stands in for both (the rewrite below points the security pocket at it as well).
    base="$candidate"
    break
  fi
done

if [ -z "$base" ]; then
  echo "::warning::apt-mirror-select: no candidate Ubuntu mirror resolves; leaving apt sources unchanged"
  exit 0
fi

shopt -s nullglob
files=(/etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources)
for f in "${files[@]}"; do
  [ -f "$f" ] || continue
  sudo sed -i -E "s#https?://(azure\.archive|archive|security)\.ubuntu\.com/ubuntu/?#${base}/#g" "$f"
done
if [ -f /etc/apt/apt-mirrors.txt ]; then
  echo "${base}/" | sudo tee /etc/apt/apt-mirrors.txt >/dev/null
fi
echo "::notice::apt-mirror-select: apt now uses ${base} (archive and security pockets)"
