import { selectCounts, type WorkflowState } from "@/lib/batch/workflow";

export function OverallProgress({ state }: { state: WorkflowState }) {
  const counts = selectCounts(state.jobs);
  const total = state.jobs.length;
  const currentNumber = Math.min(counts.completed + counts.failed + counts.skipped + 1, total);

  return (
    <div className="space-y-3">
      <p className="text-sm font-medium">
        Processing {currentNumber} of {total} videos
      </p>
      <dl className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
        <div className="flex items-center gap-1">
          <dt className="sr-only">Completed</dt>
          <dd>{counts.completed} completed</dd>
        </div>
        <div className="flex items-center gap-1">
          <dt className="sr-only">Failed</dt>
          <dd>{counts.failed} failed</dd>
        </div>
        <div className="flex items-center gap-1">
          <dt className="sr-only">Skipped</dt>
          <dd>{counts.skipped} skipped</dd>
        </div>
        <div className="flex items-center gap-1">
          <dt className="sr-only">Pending</dt>
          <dd>{counts.pending} pending</dd>
        </div>
      </dl>
    </div>
  );
}
