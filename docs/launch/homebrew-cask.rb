# Homebrew Cask formula for 4DA
# Submit to: https://github.com/Homebrew/homebrew-cask/blob/master/CONTRIBUTING.md
#
# After first GitHub release with DMG artifacts:
# 1. Get the DMG URL from GitHub releases
# 2. Calculate sha256: shasum -a 256 4DA-Home_1.0.0_aarch64.dmg
# 3. Update url and sha256 below
# 4. Submit PR to homebrew-cask
#
# Users install with: brew install --cask 4da

cask "4da" do
  arch arm: "aarch64", intel: "x86_64"

  version "1.0.0"
  sha256 arm:   "REPLACE_WITH_AARCH64_SHA256",
         intel: "REPLACE_WITH_X86_64_SHA256"

  url "https://github.com/runyourempire/4DA/releases/download/v#{version}/4DA-Home_#{version}_#{arch}.dmg",
      verified: "github.com/runyourempire/4DA/"
  name "4DA"
  desc "Privacy-first developer intelligence — scores content against your codebase"
  homepage "https://4da.ai"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :catalina"

  app "4DA.app"

  zap trash: [
    "~/Library/Application Support/com.4da.app",
    "~/Library/Caches/com.4da.app",
    "~/Library/Preferences/com.4da.app.plist",
    "~/Library/Saved Application State/com.4da.app.savedState",
  ]
end
