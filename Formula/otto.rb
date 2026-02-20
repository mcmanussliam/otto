class Otto < Formula
  desc "Task runner with retries, timeouts, history, and notifications"
  homepage "https://github.com/mcmanussliam/otto"
  url "https://github.com/mcmanussliam/otto/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "8cd75c7f90e9391d000ea4202db7ca7c201e0d090a6c9c08f162e6d3ace39c56"
  license "MIT"
  version "1.0.0"

  depends_on "go" => :build

  def install
    ldflags = %W[
      -s -w
      -X github.com/mcmanussliam/otto/internal/version.Value=v#{version}
    ]

    system "go", "build", *std_go_args(ldflags: ldflags), "./cmd/otto"
  end

  test do
    assert_match(/[[:graph:]]+/, shell_output("#{bin}/otto version"))
  end
end
