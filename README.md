# Ding

A tiny desktop app for a headphone-wearing team. You show up on the local network, tap a coworker, and a bell (or a knock, or an airhorn) pops up on their screen.

No accounts. No database. Each app hosts its own little server and finds other people with mDNS.

## Install from source

You need a recent [Rust](https://rustup.rs/) toolchain (`rustup` is enough).

### Ubuntu

```bash
sudo apt install pkg-config libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
git clone https://github.com/adel-hadadi/ding.git
cd ding
cargo run --release -p ding-client --bin ding-gui -- YourName
```

### macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
git clone https://github.com/adel-hadadi/ding.git
cd ding
cargo run --release -p ding-client --bin ding-gui -- YourName
```

Apple Silicon and Intel both work. Build on the Mac you will run it on, or grab the matching artifact below.

Leave off `YourName` and the window will ask.

### Terminal client

Same crate, no window:

```bash
cargo run --release -p ding-client --bin ding -- YourName
```

## Prebuilt binaries (GitHub Actions)

Every push to `main` builds release binaries and uploads them as artifacts:

| Artifact | Who it’s for |
| --- | --- |
| `ding-linux-x64` | Ubuntu / other x86_64 Linux |
| `ding-macos-arm64` | Apple Silicon (M1 and later) |
| `ding-macos-x64` | Intel Macs |

Open the repo → **Actions** → latest green **Build** run → **Artifacts**, then unzip. Each zip has:

- `ding-gui` — the window
- `ding` — the terminal client
- `ding-server` — optional standalone server (the apps already embed one)

```bash
chmod +x ding-gui
./ding-gui YourName
```

On a Mac the first launch is blocked as unsigned. Right-click the file → **Open**, or:

```bash
xattr -d com.apple.quarantine ding-gui ding ding-server
```

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
```

Binaries land in `target/release/ding` and `target/release/ding-gui`.

CI is the GitHub Action in `.github/workflows/build.yml`. It tests on Linux and builds Linux + macOS (Apple Silicon and Intel) release artifacts.
