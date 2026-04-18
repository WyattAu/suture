class Suture < Formula
  desc "Version control system that understands your file formats"
  homepage "https://github.com/WyattAu/suture"
  version "3.0.0"
  license "Apache-2.0"

  on_macos do
    on_intel do
      url "https://github.com/WyattAu/suture/releases/download/v3.0.0/suture-x86_64-macos.tar.gz"
      sha256 "PLACEHOLDER_MACOS_X86_64"
    end
    on_arm do
      url "https://github.com/WyattAu/suture/releases/download/v3.0.0/suture-aarch64-macos.tar.gz"
      sha256 "PLACEHOLDER_MACOS_AARCH64"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/WyattAu/suture/releases/download/v3.0.0/suture-x86_64-linux.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X86_64"
    end
    on_arm do
      url "https://github.com/WyattAu/suture/releases/download/v3.0.0/suture-aarch64-linux.tar.gz"
      sha256 "PLACEHOLDER_LINUX_AARCH64"
    end
  end

  def install
    bin.install "suture"
  end

  test do
    assert_match "suture", shell_output("#{bin}/suture --version")
    assert_match "Usage", shell_output("#{bin}/suture --help")
  end
end
