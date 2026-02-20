class Otto < Formula
  desc "Task runner with retries, timeouts, history, and notifications"
  homepage "https://github.com/mcmanussliam/otto"
  url "https://github.com/mcmanussliam/otto/archive/refs/heads/main.tar.gz"
  version "main"
  sha256 :no_check
  license "MIT"

  depends_on "go" => :build

  def install
    ldflags = %W[
      -s -w
      -X github.com/mcmanussliam/otto/internal/version.Value=dev
    ]

    system "go", "build", *std_go_args(ldflags: ldflags), "./cmd/otto"
  end

  test do
    assert_match(/[[:graph:]]+/, shell_output("#{bin}/otto version"))
  end
end
