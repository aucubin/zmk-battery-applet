# Template for the cask in aucubin/homebrew-tap.
# The release workflow fills in @VERSION@ and @SHA256@ and pushes the result.
cask "zmk-battery-applet" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/aucubin/zmk-battery-applet/releases/download/v#{version}/zmk-battery-applet-macos-universal.zip"
  name "ZMK Battery"
  desc "Menu bar applet showing the battery level of ZMK split keyboards"
  homepage "https://github.com/aucubin/zmk-battery-applet"

  depends_on :macos

  app "ZMK Battery.app"

  # The app is only ad-hoc signed, so Gatekeeper would report it as damaged.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/ZMK Battery.app"]
  end

  uninstall quit: "de.aucubin.zmk-battery-applet"
end
