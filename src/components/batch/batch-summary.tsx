import { AlertTriangle, CheckCircle2, RotateCcw } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { formatCount } from "@/lib/batch/formatters";
import { selectCounts, type WorkflowState } from "@/lib/batch/workflow";

interface BatchSummaryViewProps {
  state: WorkflowState;
  onRestart: () => void;
  onViewFailures: () => void;
}

export function BatchSummaryView({
  state,
  onRestart,
  onViewFailures,
}: BatchSummaryViewProps) {
  const counts = selectCounts(state.jobs);
  const slides = state.jobs.reduce((total, job) => total + job.slidesDetected, 0);
  const outputRoot = state.summary?.outputRoot ?? state.outputRoot;

  return (
    <div className="space-y-6">
      {state.cancelled ? (
        <Alert variant="warning">
          <AlertTriangle aria-hidden="true" />
          <AlertTitle>Processing cancelled</AlertTitle>
          <AlertDescription>
            Completed PDFs have been preserved. {counts.cancelled} videos were not processed.
          </AlertDescription>
        </Alert>
      ) : counts.failed > 0 ? (
        <Alert variant="warning">
          <AlertTriangle aria-hidden="true" />
          <AlertTitle>Finished with failures</AlertTitle>
          <AlertDescription>
            {counts.failed === 1 ? "1 video failed" : `${counts.failed} videos failed`}. Successful
            PDFs were preserved.
          </AlertDescription>
        </Alert>
      ) : (
        <Alert variant="success">
          <CheckCircle2 aria-hidden="true" />
          <AlertTitle>Batch complete</AlertTitle>
          <AlertDescription>
            All videos were processed and the folder structure was preserved.
          </AlertDescription>
        </Alert>
      )}

      <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm sm:grid-cols-3">
        <Stat label="Videos found" value={formatCount(state.jobs.length)} />
        <Stat label="Completed" value={formatCount(counts.completed)} />
        <Stat label="Failed" value={formatCount(counts.failed)} />
        <Stat label="Skipped" value={formatCount(counts.skipped)} />
        <Stat label="Cancelled" value={formatCount(counts.cancelled)} />
        <Stat label="Slides detected" value={formatCount(slides)} />
      </dl>

      <Separator />

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">Output</p>
        <p className="break-all text-sm">{outputRoot}</p>
      </div>

      <div className="flex flex-wrap gap-3">
        {counts.failed > 0 ? (
          <Button type="button" variant="outline" onClick={onViewFailures}>
            View failed items
          </Button>
        ) : null}
        <Button type="button" variant="outline" onClick={onRestart}>
          <RotateCcw aria-hidden="true" />
          Process another folder
        </Button>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-lg font-semibold tabular-nums">{value}</dd>
    </div>
  );
}
