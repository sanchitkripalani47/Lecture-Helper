# Lecture Helper

Turns recorded lecture videos into slide PDFs. Point it at a folder, or at a single video, and it finds the videos, detects each slide change, and writes one PDF per lecture — keeping the original folder structure.

Everything runs locally. Nothing is uploaded.

## Install

**1. Install ffmpeg.** The app uses it to read video, and does not bundle it:

- **macOS** — `brew install ffmpeg`
- **Windows** — `winget install Gyan.FFmpeg`, then reopen your terminal

**2. Download the installer** for your platform from the [latest release](../../releases/latest):

- **macOS** — open the `.dmg` and drag Lecture Helper into Applications
- **Windows** — run the `.msi` or `-setup.exe`

**3. Let your system run it.** The builds are not code signed, so each system warns once:

- **macOS** — right-click the app and choose **Open**; or run

  ```bash
  xattr -dr com.apple.quarantine "/Applications/Lecture Helper.app"
  ```

- **Windows** — SmartScreen says "Windows protected your PC": choose **More info**, then **Run anyway**

## Use

1. Choose a folder, or a single video.
2. Choose where the PDFs go: a new folder, or next to the videos.
3. Review the list and start.

## Build from source

Needs Node.js 20+, Rust, and ffmpeg on `PATH`.

```bash
npm install
npm run tauri build
```

Installers appear in `src-tauri/target/release/bundle/`.

[DEVELOPMENT.md](./DEVELOPMENT.md) covers setup and testing in detail;
[product-description.md](./product-description.md) holds the product requirements.
