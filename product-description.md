# Batch Folder Processing

The application must support processing an entire folder hierarchy, not just individual video files.

This is a core V1 requirement.

## User workflow

The user should be able to select a root folder such as:

```text
My Videos/
```

The application must recursively scan that folder and find all supported video files.

Example:

```text
My Videos/
│
├── Course B/
│   ├── Lecture 01.mp4
│   ├── Lecture 02.mp4
│   └── Lecture 03.mov
│
├── Course A/
│   ├── Lecture 01.mp4
│   └── Lecture 02.mkv
│
└── Course C/
    ├── Lecture 01.mp4
    └── Lecture 02.mp4
```

The application should discover all supported videos recursively.

Supported extensions should include at least:

```text
.mp4
.mov
.mkv
.avi
.webm
```

Do not require the user to manually select each video.

---

# Batch processing behavior

After selecting a root folder:

1. Recursively scan all subdirectories.
2. Identify supported video files.
3. Ignore unsupported files.
4. Build a processing queue.
5. Display the discovered videos before processing.
6. Allow the user to start processing.
7. Process videos sequentially by default.
8. Generate a PDF for each video.
9. Preserve the original directory hierarchy.
10. Continue processing if one video fails.
11. Report successful and failed videos at the end.

Example:

```text
Found 27 videos

Course B
  ✓ Lecture 01.mp4
  ✓ Lecture 02.mp4
  ✓ Lecture 03.mp4

Course A
  ✓ Lecture 01.mp4
  ✓ Lecture 02.mp4

...
```

---

# Preserve directory structure

The output must preserve the relative directory structure of the input folder.

Given:

```text
Input/
├── Course A/
│   ├── Lecture 01.mp4
│   └── Lecture 02.mp4
│
└── Course B/
    └── Lecture 01.mp4
```

The output should be:

```text
Output/
├── Course A/
│   ├── Lecture 01.pdf
│   └── Lecture 02.pdf
│
└── Course B/
    └── Lecture 01.pdf
```

Do NOT flatten all PDFs into one directory.

This is important because the user may process hundreds of lectures belonging to different courses.

---

# Output directory

When the user selects:

```text
My Videos/
```

the application should allow them to choose an output directory.

Default behavior should preferably be:

```text
My Videos/
My Videos - Processed/
```

rather than modifying the original video directory.

Never overwrite the original video files.

The user should be able to change the output location.

---

# File naming

By default:

```text
Lecture 01.mp4
        ↓
Lecture 01.pdf
```

Preserve the original filename and replace the extension with `.pdf`.

Do not rename files unnecessarily.

Handle filenames containing:

- spaces
- parentheses
- Unicode characters
- multiple dots
- non-English characters

correctly.

---

# Batch progress UI

When processing multiple videos, the UI should show both:

### Overall progress

```text
Processing 7 of 27 videos

██████████░░░░░░░░░░ 26%
```

### Current video

```text
Course B / Lecture 07.mp4

Detecting slides...

██████████████░░░░░░ 72%

Slides detected: 38
```

Do not fake progress.

The processing engine should emit real progress/state information.

---

# Processing queue

Represent each discovered video as a job.

For example:

```python
VideoJob(
    source_path=...,
    relative_path=...,
    output_path=...,
    status=...,
    progress=...,
    slides_detected=...,
    error=...
)
```

Possible statuses:

```text
pending
processing
completed
failed
cancelled
skipped
```

Keep the queue abstraction independent of the current PDF-only implementation.

This is important because future processing will involve more outputs.

---

# Continue on failure

If one video fails:

```text
Lecture 01 → ✓
Lecture 02 → ✓
Lecture 03 → ✗
Lecture 04 → pending
```

the application should NOT stop the entire batch.

It should:

1. Record the failure.
2. Display/log the error.
3. Continue with Lecture 04.

At the end, show a summary:

```text
Processing complete

✓ 24 completed
✗ 2 failed
○ 1 skipped

[ View Failed Items ]
[ Open Output Folder ]
```

---

# Resume / skip already processed videos

Design the batch system so that it can eventually resume interrupted work.

For V1, implement at least:

> If the expected output PDF already exists, allow the user to skip that video.

For example:

```text
Lecture 01.mp4
Lecture 01.pdf already exists

○ Reprocess
● Skip
```

For an entire batch, provide a simple option:

```text
When output already exists:

● Skip existing
○ Replace existing
```

Do not overwrite existing PDFs by default.

---

# Cancellation

The user must be able to cancel batch processing.

If the user clicks:

```text
[ Cancel ]
```

the application should:

1. Stop starting new jobs.
2. Allow the currently running operation to terminate safely.
3. Clean temporary files.
4. Preserve completed outputs.
5. Mark remaining jobs as cancelled/pending.

Example:

```text
Processed: 12 / 40

12 completed
1 processing
27 pending

[ Cancel Processing ]
```

After cancellation:

```text
Processing cancelled

✓ 12 completed
○ 27 not processed

The completed PDFs have been preserved.
```

---

# Concurrency

Do NOT process every video simultaneously by default.

Video processing is CPU/memory intensive.

V1 should process videos sequentially:

```text
Lecture 01
    ↓
Lecture 02
    ↓
Lecture 03
    ↓
...
```

Architect the queue so controlled concurrency can be added later if useful.

Do not sacrifice stability for parallel processing.

---

# Future transcription/OCR compatibility

The batch-processing architecture must NOT assume that each video produces only one PDF.

Currently:

```text
Lecture 01.mp4
       ↓
Lecture 01.pdf
```

Future versions may produce:

```text
Lecture 01.mp4
       ↓
Lecture 01/
   ├── slides.pdf
   ├── transcript.txt
   ├── transcript.json
   └── slide-text.json
```

Therefore, keep the concept of:

```text
VideoJob → ProcessingResult
```

rather than hard-coding:

```text
VideoJob → PDF
```

For example:

```python
ProcessingResult(
    source_video=...,
    slides_pdf=...,
    transcript=...,
    slide_text=...,
    metadata=...
)
```

For V1, only `slides_pdf` needs to be populated.

The other fields can be optional/not implemented yet.

---

# Future output organization

The architecture should support eventually generating:

```text
Processed/
│
├── Course B/
│   │
│   ├── Lecture 01/
│   │   ├── slides.pdf
│   │   ├── transcript.txt
│   │   ├── transcript.json
│   │   └── slide-text.json
│   │
│   ├── Lecture 02/
│   │   ├── slides.pdf
│   │   ├── transcript.txt
│   │   └── slide-text.json
│   │
│   └── Lecture 03/
│       └── ...
│
└── Course A/
    └── ...
```

Do NOT implement this expanded output format in V1 unless necessary.

V1 can simply produce:

```text
Processed/
├── Course B/
│   ├── Lecture 01.pdf
│   └── Lecture 02.pdf
└── Course A/
    └── Lecture 01.pdf
```

But make sure the architecture does not prevent the future structure.

---

# Batch summary

At the end of processing, provide a summary:

```text
Batch complete

Videos found:     27
Completed:        25
Failed:            2
Skipped:           0

Slides detected:  1,243

Output:
My Videos - Processed/

[ Open Output Folder ]
```

The exact aggregate statistics can be expanded later.

---

# Important requirement

Batch processing must remain local.

Do not upload the root folder or any video to a server.

Do not create a database just to maintain the batch queue.

The queue should exist in memory during processing, with optional lightweight local metadata only if needed for resume functionality.

The application must remain lightweight and offline-capable.
