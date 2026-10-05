import type { LucideIcon } from "lucide-react";
import { CheckCircle2, Circle, CircleSlash, Loader2, MinusCircle, XCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { JOB_STATUS_LABEL, type JobStatus } from "@/lib/batch/types";
import { cn } from "@/lib/utils";

const STATUS_ICON: Record<JobStatus, LucideIcon> = {
  pending: Circle,
  processing: Loader2,
  completed: CheckCircle2,
  failed: XCircle,
  cancelled: CircleSlash,
  skipped: MinusCircle,
};

const STATUS_VARIANT: Record<
  JobStatus,
  "default" | "secondary" | "success" | "destructive" | "muted" | "outline"
> = {
  pending: "muted",
  processing: "secondary",
  completed: "success",
  failed: "destructive",
  cancelled: "muted",
  skipped: "outline",
};

export function StatusBadge({ status, className }: { status: JobStatus; className?: string }) {
  const Icon = STATUS_ICON[status];
  return (
    <Badge variant={STATUS_VARIANT[status]} className={cn("shrink-0", className)}>
      <Icon
        aria-hidden="true"
        className={cn("h-3 w-3", status === "processing" && "motion-safe:animate-spin")}
      />
      {JOB_STATUS_LABEL[status]}
    </Badge>
  );
}
