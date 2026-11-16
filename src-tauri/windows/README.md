The custom NSIS template is the official Tauri CLI 2.12.4 template from:
https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi

Original SHA256: d00089d3b1a2c4a30d7b6e098079100cb8f25e980f84d4515aca4597b95e967d
Tauri's MIT license is included as LICENSE-TAURI-MIT.

Local changes define the stable `INSTALLIDENTITY` as `Sonora` and use it for
the uninstall key and manufacturer/product key. The visible PRODUCTNAME
remains `Song Player`. This preserves version/upgrade detection and restores
the existing install directory when upgrading from Sonora. The app bundle ID,
SQLite name, environment override, RegisteredApplications key and audio ProgID
also remain unchanged.

hooks.nsh migrates old Sonora and Audio Player shortcuts in the Start menu and
on the desktop only after verifying that they target the same installed executable.
If a Song Player shortcut points elsewhere, both shortcuts are preserved. A full
uninstall also removes retained legacy shortcuts only after the same target check;
update mode retains them for migration. File association registration advertises
formats through OpenWith only; Windows retains control of user default associations.

When updating the Tauri CLI, compare this template against that exact upstream
version and carry forward only the stable identity changes.
