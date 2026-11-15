class Solus < Formula
  desc "High-definition terminal anime theatre, discovery crucible, and AniList/MAL companion"
  homepage "https://github.com/abdullah-binmadhi/Solus"
  version "1.0.7-rolling"

  if Hardware::CPU.arm?
    url "https://github.com/abdullah-binmadhi/Solus/releases/download/v1.0.7-rolling/solus-macos-aarch64"
  else
    url "https://github.com/abdullah-binmadhi/Solus/releases/download/v1.0.7-rolling/solus-macos-x86_64"
  end

  depends_on "ani-cli"
  depends_on "mpv"

  def install
    bin.install Dir["solus-macos-*"].first => "solus"
  end

  def caveats
    <<~EOS
      Solus requires mpv and ani-cli for media playback:

        brew install mpv ani-cli

      For the best experience with full cover art and episode previews, run Solus in Kitty or WezTerm:
        brew install --cask kitty

      In the app:
      - press s for Settings & AniList Sync
      - press ! for Setup / dependency status
      - press ? for keyboard shortcuts
    EOS
  end

  test do
    assert_predicate bin/"solus", :exist?
  end
end
