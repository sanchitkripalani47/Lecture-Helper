# Development

Lecture Helper converts lecture videos into slide PDFs. It runs entirely on the
local machine: no server, no uploads, no database.

The user-facing overview is [README.md](./README.md) and the product
requirements live in [product-description.md](./product-description.md).

## Prerequisites

Both platforms need Node.js 20+, Rust (stable) with Cargo, and `ffmpeg` +
`ffprobe` on `PATH`.

Installing ffmpeg:

```bash
# macOS
brew install ffmpeg

# Windows (winget, then reopen your terminal)
winget install Gyan.FFmpeg
# or download a build from https://www.gyan.dev/ffmpeg/builds/ and add its
# bin folder to PATH
```

Windows additionally needs the **Microsoft C++ Build Tools** (Visual Studio
Installer → "Desktop development with C++"), because Rust links with MSVC. The
WebView2 runtime is required at run time and already ships with Windows 11 and
current Windows 10.

The absence of `ffmpeg`/`ffprobe` is detected before a batch starts and reported
in the UI, with a platform-appropriate hint, so the app still launches without
it.

## Getting started

```bash
npm install
npm run dev          # browser preview (uses a built-in simulator)
npm run tauri dev    # the real desktop app
```

`npm run dev` runs outside Tauri, where a mock engine
(`createMockClient` in `src/lib/batch/tauri-client.ts`) provides a sample queue
and streams simulated progress. This lets you work on the UI without Rust or
ffmpeg. The desktop build always uses the real engine.

## Building an installer

Tauri cannot cross-compile, so build on each machine you want to target.

```bash
npm run icons        # regenerate icons, including the Windows .ico
npm run tauri build  # compile and package for the current platform
```

Artifacts land in `src-tauri/target/release/bundle/`:

| Platform | Artifacts                          |
| -------- | ---------------------------------- |
| macOS    | `dmg/*.dmg` and `macos/*.app`      |
| Windows  | `msi/*.msi` and `nsis/*-setup.exe` |
| Linux    | `deb/`, `rpm/`, `appimage/`        |

Installing the result:

- **macOS** — open the `.dmg` and drag the app to Applications. A locally built
  app has no quarantine flag and just runs. If you copy a build to another Mac,
  Gatekeeper will block it; either right-click the app and choose **Open**, or
  clear the flag:

  ```bash
  xattr -dr com.apple.quarantine "/Applications/Lecture Helper.app"
  ```

- **Windows** — run the `.msi` or `-setup.exe`. Because it is unsigned,
  SmartScreen shows "Windows protected your PC": choose **More info**, then
  **Run anyway**. This appears once per machine.

Neither build is code signed or notarized, which is why the two warnings above
exist. Signing would remove them but requires paid Apple and Microsoft
certificates.

To hand someone a single build that runs on both Intel and Apple Silicon Macs:

```bash
rustup target add x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

## Checks

```bash
npm run typecheck    # tsc --noEmit
npm test             # vitest: reducers, formatters, and component flows
npm run build        # typecheck + production bundle
npm run format       # prettier

cd src-tauri
cargo test           # backend unit + integration tests
cargo check          # type check the Tauri crate
cargo fmt            # rustfmt
cargo clippy         # lints
```

## Architecture

```text
src/                          React UI (presentation only)
  components/batch/           Workflow screens and pieces
  components/ui/              shadcn/ui primitives
  lib/batch/
    types.ts                  DTOs mirroring the Rust models
    workflow.ts               Reducer: stages, counts, progress
    grouping.ts               Queue grouping and summaries
    tauri-client.ts           The only bridge to Tauri

src-tauri/src/                Rust backend (all I/O and processing)
  domain/                     Models, errors, skip/replace policy
  discovery/                  Recursive scan and output path mapping
  processing/                 One video at a time
    media.rs                  ffmpeg/ffprobe + slide detection
    pdf.rs                    Hand-rolled JPEG-embedding PDF writer
    temp_outputs.rs           Atomic `<name>.pdf.partial` -> `<name>.pdf`
    cancellation.rs           Cooperative cancellation token
  batch/                      Sequential queue runner + summary
  infrastructure/             Subprocess and temp-directory helpers
  commands.rs                 Tauri command surface
```

### Key invariants

- **The queue is output-agnostic.** `VideoJob` → `ProcessingResult`, never
  `VideoJob` → PDF. Adding transcripts or OCR will not change the queue model.
- **Never write the destination directly.** Output goes to a sibling `.partial`
  file and is renamed only on success.
- **Continue on failure.** One bad video is recorded and the batch moves on.
- **Cancellation is cooperative.** New jobs stop, the active video exits safely,
  completed PDFs are preserved, and the rest are marked cancelled.
- **Progress is real.** Percentages come from ffmpeg's own progress stream and
  the slide-detection loop; there are no timers driving the bars.
- **The source tree is never modified.** Only two output locations are rejected:
  one that would overwrite a source video, and the lecture folder itself.

### Frontend/backend contract

The UI calls three commands and listens to one event stream:

| Name            | Direction | Purpose                                    |
| --------------- | --------- | ------------------------------------------ |
| `discover_jobs` | invoke    | Scan a folder and return `ScanResult`      |
| `start_batch`   | invoke    | Validate dependencies and start processing |
| `cancel_batch`  | invoke    | Request cooperative cancellation           |
| `batch://event` | event     | `BatchEvent` progress and state updates    |

Envelope types are defined once in Rust (`domain/models.rs`) with
`#[serde(rename_all = "camelCase")]` and mirrored in `src/lib/batch/types.ts`.
Keep the two in step when changing either.

## Icons

`src-tauri/icons` holds generated placeholders. Replace them with real artwork:

```bash
npm run tauri icon path/to/source.png
```

## Known limitations (V1)

- Sequential processing only; no parallelism.
- No resume across application restarts. Existing PDFs are skipped by default.
- No OCR, transcription, or per-video output folders, though `ProcessingResult`
  already has the optional fields for them.
- Slide detection uses a fixed sampling interval and luma-difference threshold
  (`SlideDetectionOptions`); they are not yet user-configurable.
