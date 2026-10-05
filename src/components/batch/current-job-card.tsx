import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { basename } from "@/lib/batch/formatters";
import type { VideoJob } from "@/lib/batch/types";
import { StatusBadge } from "./status-badge";

export function CurrentJobCard({ job }: { job: VideoJob | null }) {
  if (!job) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Current video</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Waiting for the next video.</p>
        </CardContent>
      </Card>
    );
  }

  const name = basename(job.relativePath);

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="truncate">{name}</CardTitle>
        <StatusBadge status={job.status} />
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-sm text-muted-foreground">{job.stage}</p>
        <dl className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <dt className="text-xs text-muted-foreground">Slides detected</dt>
            <dd className="tabular-nums">{job.slidesDetected}</dd>
          </div>
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">Output</dt>
            <dd className="truncate" title={job.outputPath}>
              {job.outputPath}
            </dd>
          </div>
        </dl>
      </CardContent>
    </Card>
  );
}
