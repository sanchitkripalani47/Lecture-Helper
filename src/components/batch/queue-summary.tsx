import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { selectCounts, type WorkflowState } from "@/lib/batch/workflow";
import type { ExistingOutputPolicy } from "@/lib/batch/types";
import { PolicySelector } from "./policy-selector";
import { QueueTree } from "./queue-tree";

interface QueueSummaryProps {
  state: WorkflowState;
  policy: ExistingOutputPolicy;
  outputRoot: string;
  onPolicyChange: (policy: ExistingOutputPolicy) => void;
  onStart: () => void;
  onBack: () => void;
}

export function QueueSummary({
  state,
  policy,
  outputRoot,
  onPolicyChange,
  onStart,
  onBack,
}: QueueSummaryProps) {
  const counts = selectCounts(state.jobs);
  const existingCount = state.jobs.filter((job) => job.outputExists).length;
  const videoLabel = state.jobs.length === 1 ? "1 video" : `${state.jobs.length} videos`;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-sm font-medium">Found {videoLabel}</p>
        <Badge variant="muted">{counts.pending} pending</Badge>
        {existingCount > 0 ? (
          <Badge variant="outline">{existingCount} already processed</Badge>
        ) : null}
      </div>

      {existingCount > 0 ? (
        <Alert>
          <AlertTitle>Existing output found</AlertTitle>
          <AlertDescription>
            {existingCount === 1
              ? "1 video already has a PDF"
              : `${existingCount} videos already have PDFs`}{" "}
            in the output folder.
          </AlertDescription>
        </Alert>
      ) : null}

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">Saving to</p>
        <p className="break-all text-sm">{outputRoot}</p>
      </div>

      <PolicySelector policy={policy} onPolicyChange={onPolicyChange} />

      <QueueTree jobs={state.jobs} />

      <div className="flex flex-wrap gap-3">
        <Button type="button" onClick={onStart}>
          Start processing
        </Button>
        <Button type="button" variant="outline" onClick={onBack}>
          Back
        </Button>
      </div>
    </div>
  );
}
