import type { JobStatus } from "./types";

export function basename(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index === -1 ? normalized : normalized.slice(index + 1);
}

export function formatCount(value: number): string {
  return new Intl.NumberFormat().format(value);
}

export function formatProgressLabel(completed: number, total: number): string {
  return `Processing ${completed} of ${total} videos`;
}

export function statusDescription(status: JobStatus): string {
  switch (status) {
    case "pending":
      return "Waiting to process";
    case "processing":
      return "Processing now";
    case "completed":
      return "PDF created";
    case "failed":
      return "Processing failed";
    case "cancelled":
      return "Not processed";
    case "skipped":
      return "Skipped, output already exists";
  }
}
