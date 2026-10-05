import { Badge } from "@/components/ui/badge";
import { basename } from "@/lib/batch/formatters";
import type { VideoJob } from "@/lib/batch/types";
import { StatusBadge } from "./status-badge";

export function QueueItem({ job }: { job: VideoJob }) {
  const name = basename(job.relativePath);

  return (
    <li className="flex items-start gap-3 px-3 py-2.5">
      <StatusBadge status={job.status} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <p className="truncate text-sm font-medium">{name}</p>
          {job.outputExists ? (
            <Badge variant="outline" className="shrink-0 text-[11px]">
              Output exists
            </Badge>
          ) : null}
        </div>
        <p className="truncate text-xs text-muted-foreground">{job.outputPath}</p>

        <p className="mt-0.5 text-xs text-muted-foreground">
          {job.error ? (
            <span className="text-destructive">{job.error}</span>
          ) : job.status === "processing" ? (
            `${job.stage}${job.slidesDetected > 0 ? `, ${job.slidesDetected} slides` : ""}`
          ) : (
            job.stage
          )}
        </p>
      </div>
    </li>
  );
}
