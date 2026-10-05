import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { basename } from "@/lib/batch/formatters";
import type { VideoJob } from "@/lib/batch/types";

interface FailedItemsDialogProps {
  jobs: VideoJob[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function FailedItemsDialog({ jobs, open, onOpenChange }: FailedItemsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Failed items</DialogTitle>
          <DialogDescription>
            These videos were not converted. Successful PDFs were preserved.
          </DialogDescription>
        </DialogHeader>
        <ul className="max-h-[360px] space-y-3 overflow-y-auto">
          {jobs.map((job) => (
            <li key={job.id} className="rounded-md border p-3">
              <p className="text-sm font-medium">{basename(job.relativePath)}</p>
              <p className="break-all text-xs text-muted-foreground">{job.sourcePath}</p>
              <p className="mt-1 text-sm text-destructive">{job.error ?? "Unknown error"}</p>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}
