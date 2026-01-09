# frozen_string_literal: true

# Homebrew formula for AGIT
# To use: Copy this file to agit-stuff/homebrew-agit/Formula/agit.rb
#
# Usage:
#   brew tap agit-stuff/agit
#   brew install agit

class Agit < Formula
  desc "AI-native Git wrapper for capturing context alongside code"
  homepage "https://github.com/agit-stuff/agit"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_intel do
      url "https://github.com/agit-stuff/agit/releases/download/v#{version}/agit-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_HASH_TO_BE_UPDATED_BY_CI"
    end

    on_arm do
      url "https://github.com/agit-stuff/agit/releases/download/v#{version}/agit-macos-aarch64.tar.gz"
      sha256 "PLACEHOLDER_HASH_TO_BE_UPDATED_BY_CI"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agit-stuff/agit/releases/download/v#{version}/agit-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_HASH_TO_BE_UPDATED_BY_CI"
    end

    on_arm do
      url "https://github.com/agit-stuff/agit/releases/download/v#{version}/agit-linux-aarch64.tar.gz"
      sha256 "PLACEHOLDER_HASH_TO_BE_UPDATED_BY_CI"
    end
  end

  def install
    bin.install "agit"
  end

  test do
    assert_match "agit #{version}", shell_output("#{bin}/agit --version")
  end
end
