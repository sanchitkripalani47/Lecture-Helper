import type { BatchSummary, VideoJob } from "./types";

export function computeSummary(jobs: VideoJob[], outputRoot: string): BatchSummary {
  return {
    found: jobs.length,
    completed: jobs.filter((job) => job.status === "completed").length,
    failed: jobs.filter((job) => job.status === "failed").length,
    skipped: jobs.filter((job) => job.status === "skipped").length,
    cancelled: jobs.filter((job) => job.status === "cancelled").length,
    slidesDetected: jobs.reduce((total, job) => total + job.slidesDetected, 0),
    outputRoot,
  };
}

export interface QueueGroup {
  directory: string;
  jobs: VideoJob[];
}

export function groupJobsByDirectory(jobs: VideoJob[]): QueueGroup[] {
  const groups = new Map<string, VideoJob[]>();
  for (const job of jobs) {
    const relative = job.relativePath.replace(/\\/g, "/");
    const lastSlash = relative.lastIndexOf("/");
    const directory = lastSlash === -1 ? "." : relative.slice(0, lastSlash);
    const bucket = groups.get(directory);
    if (bucket) {
      bucket.push(job);
    } else {
      groups.set(directory, [job]);
    }
  }

  return Array.from(groups.entries())
    .map(([directory, grouped]) => ({ directory, jobs: grouped }))
    .sort((a, b) => a.directory.localeCompare(b.directory));
}
