# Proposed upstream message

Suggested title: **Thank you — optional Raspberry Pi / uConsole ARM64 installer**

Hi Abdullah,

Thank you for creating Solus and sharing it as open source! I have been using it
on a Raspberry Pi Compute Module 5 uConsole running Debian 13 ARM64. I built the
existing Rust application natively and confirmed that the catalog and cover art
display in both Foot and Kitty.

I prepared an optional Raspberry Pi installer with a choice of Foot, Kitty, or
the existing terminal, a memory-conscious native build script, and optional USB
download storage identified by filesystem UUID. The additions are isolated under
`scripts/raspberry-pi/` with a guide and a small README section. They do not modify
the Rust application, Cargo.lock, original installers or release workflow, so the
change should be straightforward to review independently.

The installer checks were exercised with a previously built ARM64 binary in a
temporary HOME. A fresh installation on every Raspberry Pi model has not been
tested, and hardware video decoding is left to mpv and the installed drivers.

I would be happy to contribute these scripts if they fit the project. Thanks
again to you and everyone contributing to Solus!
