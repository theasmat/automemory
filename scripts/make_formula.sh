#!/usr/bin/env bash
# Prints a Homebrew formula for a published release, with the real checksums.
# Usage: scripts/make_formula.sh v0.1.1 > automemory.rb   (then commit it to your tap: homebrew-automemory/Formula/)
set -euo pipefail
TAG="${1:?usage: make_formula.sh vX.Y.Z}"
REPO="theasmat/automemory-bin"
sum() { curl -fsSL "https://github.com/${REPO}/releases/download/${TAG}/automemory-$1.tar.gz.sha256" | awk '{print $1}'; }
cat <<EOF
class Automemory < Formula
  desc "Shared, verified memory, locks and hand-offs for coding agents"
  homepage "https://github.com/${REPO}"
  version "${TAG#v}"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/${REPO}/releases/download/${TAG}/automemory-aarch64-apple-darwin.tar.gz"
      sha256 "$(sum aarch64-apple-darwin)"
    end
    on_intel do
      url "https://github.com/${REPO}/releases/download/${TAG}/automemory-x86_64-apple-darwin.tar.gz"
      sha256 "$(sum x86_64-apple-darwin)"
    end
  end
  on_linux do
    on_intel do
      url "https://github.com/${REPO}/releases/download/${TAG}/automemory-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$(sum x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "automemory"
  end

  test do
    assert_match "automemory", shell_output("#{bin}/automemory --version")
  end
end
EOF
