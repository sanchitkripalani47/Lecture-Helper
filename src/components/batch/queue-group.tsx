import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { QueueGroup } from "@/lib/batch/grouping";
import { cn } from "@/lib/utils";
import { QueueItem } from "./queue-item";

export function QueueGroupView({ group }: { group: QueueGroup }) {
  const [open, setOpen] = useState(true);
  const label = group.directory === "." ? "Top level" : group.directory;
  const videoLabel = group.jobs.length === 1 ? "1 video" : `${group.jobs.length} videos`;

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm font-medium hover:bg-accent">
        <ChevronRight
          aria-hidden="true"
          className={cn("h-4 w-4 text-muted-foreground transition-transform", open && "rotate-90")}
        />
        <span className="truncate">{label}</span>
        <span className="ml-auto shrink-0 text-xs font-normal text-muted-foreground">
          {videoLabel}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ul className="divide-y border-l pl-1">
          {group.jobs.map((job) => (
            <QueueItem key={job.id} job={job} />
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  );
}
