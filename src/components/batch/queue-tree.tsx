import { useMemo } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { groupJobsByDirectory } from "@/lib/batch/grouping";
import type { VideoJob } from "@/lib/batch/types";
import { QueueGroupView } from "./queue-group";

export function QueueTree({ jobs }: { jobs: VideoJob[] }) {
  const groups = useMemo(() => groupJobsByDirectory(jobs), [jobs]);

  return (
    <ScrollArea className="max-h-[420px] rounded-md border">
      <div className="space-y-1 p-2">
        {groups.map((group) => (
          <QueueGroupView key={group.directory} group={group} />
        ))}
      </div>
    </ScrollArea>
  );
}
