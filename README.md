# Ding

A tiny desktop app for a headphone-wearing team. You show up on the local network, tap a coworker, and a bell (or a knock, or an airhorn) pops up on their screen.

No accounts. No database. Each app hosts its own little server and finds other people with mDNS.

## Install as an app

After this, you open Ding from the Ubuntu app menu, macOS Launchpad, or the Windows Start menu. No terminal.

### Ubuntu

From a [Release](https://github.com/adel-hadadi/ding/releases), download `ding_*_amd64.deb` and install it:

```bash
sudo apt install ./ding_*_amd64.deb
```

Then search the app grid for **Ding** and click it. The window asks for your name.

From source:

```bash
sudo apt install pkg-config libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
git clone https://github.com/adel-hadadi/ding.git
cd ding
cargo build --release -p ding-client --bins
./packaging/linux/build-deb.sh
sudo apt install ./dist/ding_*_amd64.deb
```

Without sudo, `./packaging/linux/install.sh` copies the app into `~/.local` instead.

### macOS

Unzip the matching release (`ding-macos-arm64` on Apple Silicon, `ding-macos-x64` on Intel):

```bash
xattr -dr com.apple.quarantine Ding.app
mv Ding.app /Applications/
```

Open **Ding** from Launchpad or Applications. The first time, right-click → **Open**.

From source on a Mac:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
git clone https://github.com/adel-hadadi/ding.git
cd ding
cargo build --release -p ding-client --bin ding-gui
chmod +x packaging/macos/bundle.sh
./packaging/macos/bundle.sh target/release/ding-gui Ding.app
mv Ding.app /Applications/
```

### Windows

Unzip `ding-windows-x64.zip` from [Releases](https://github.com/adel-hadadi/ding/releases), then either:

- Double-click `ding-gui.exe`, or
- Double-click `install.bat` to put **Ding** in the Start menu

The first launch may show SmartScreen. Choose **More info** → **Run anyway**. If Windows Firewall asks, allow it on **private** networks so teammates can find you.

From source on a PC with [Rust](https://rustup.rs/):

```powershell
git clone https://github.com/adel-hadadi/ding.git
cd ding
cargo build --release -p ding-client --bins
.\target\release\ding-gui.exe
```

### Terminal client

Same crate, no window:

```bash
cargo run --release -p ding-client --bin ding -- YourName
```

## Prebuilt binaries

Tagged versions show up on [Releases](https://github.com/adel-hadadi/ding/releases):

| File | Who it’s for |
| --- | --- |
| `ding_*_amd64.deb` | Ubuntu — `sudo apt install ./ding_*_amd64.deb` |
| `ding-linux-x64.zip` | Linux binaries + `install.sh` (no root) |
| `ding-macos-arm64.zip` | Apple Silicon — `Ding.app` |
| `ding-macos-x64.zip` | Intel Macs — `Ding.app` |
| `ding-windows-x64.zip` | Windows — `ding-gui.exe` + `install.bat` |

The zips also include the terminal `ding` binary and optional `ding-server`. The apps already embed a server.

## Using it

Everyone needs to be on the **same LAN**. Guest Wi-Fi that isolates clients will hide people; that is the network, not the app.

In the window:

1. Pick a display name.
2. Nearby people show up as cards. Choose **Ding**, **Knock**, or **HONK**, then tap them.
3. An incoming ding is a banner plus a beep. **Ding back, obviously** returns fire.
4. Teams are optional. Create one, join the person who is hosting, or join by `host:port`.

In the terminal:

```
list                     who's nearby, and your team
create <team>            start a team on this computer
join <person>            join the team that person is hosting
join <host:port> <team>  join a team by address
ding <person> [kind]     ding a teammate, or someone nearby (default kind: bell)
leave
quit
```

`kind` can be `bell`, `knock`, or `airhorn`.

## Build it yourself (release)

```bash
# Ubuntu also needs: sudo apt install pkg-config libasound2-dev
cargo build --release -p ding-client --bins
./packaging/linux/build-deb.sh
sudo apt install ./dist/ding_*_amd64.deb
```

Binaries land in `target/release/`. The `.deb` lands in `dist/`.

CI is the GitHub Action in `.github/workflows/build.yml`. Pushes to `main` test and build Linux, macOS, and Windows. A tag like `v0.1.0` publishes the zips, the `.deb`, and the Windows zip on the Releases page:

```bash
git tag v0.1.0
git push origin v0.1.0
```
