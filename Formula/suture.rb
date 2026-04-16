class Suture < Formula
  desc "Version control system that understands your file formats"
  homepage "https://github.com/WyattAu/suture"
  version "1.0.0"
  license "Apache-2.0"

  on_macos do
    on_intel do
      url "https://github.com/WyattAu/suture/releases/download/v1.0.0/suture-x86_64-macos.tar.gz"
      sha256 "727ff98e22af3ec53e6ee7c5d8214f6d43cf54043d147a8e8989d18d9cdf6b35"
    end
    on_arm do
      url "https://github.com/WyattAu/suture/releases/download/v1.0.0/suture-aarch64-macos.tar.gz"
      sha256 "6ec70f8d3b6c6c71bdc022031016de40adf2aead65672f5f6c0e79c2e83034dd"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/WyattAu/suture/releases/download/v1.0.0/suture-x86_64-linux.tar.gz"
      sha256 "f5e1af7b27e4d826ce99b943c5e7b1efb0577f35e50374728aacbe3ff98b8156"
    end
    on_arm do
      url "https://github.com/WyattAu/suture/releases/download/v1.0.0/suture-aarch64-linux.tar.gz"
      sha256 "377dbaf81db73a1bf94c96640f51a20fc82099269dc673defc6f11b2a69da7dd"
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
