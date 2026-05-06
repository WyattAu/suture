class Suture < Formula
  desc "Patch-based version control system with semantic merge"
  homepage "https://github.com/WyattAu/suture"
  url "https://github.com/WyattAu/suture/archive/refs/tags/v5.1.0.tar.gz"
  sha256 "PLACEHOLDER"
  license "Apache-2.0"
  head "https://github.com/WyattAu/suture.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--path", "crates/suture-cli"
    bin.install "target/release/suture"
    bash_completion.output = Utils.safe_popen_read(bin/"suture", "completions", "bash") if (bin/"suture").exist?
    zsh_completion.output = Utils.safe_popen_read(bin/"suture", "completions", "zsh") if (bin/"suture").exist?
  end

  test do
    system bin/"suture", "version"
    (testpath/"test-repo").mkpath
    Dir.chdir(testpath/"test-repo") do
      system bin/"suture", "init", "--path", "."
      system bin/"suture", "config", "set", "user.name", "Test"
      (testpath/"test-repo/hello.txt").write("hello")
      system bin/"suture", "add", "hello.txt"
      system bin/"suture", "commit", "-m", "test commit"
      system bin/"suture", "log"
    end
  end
end
