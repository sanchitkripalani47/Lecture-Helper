import { Skeleton } from "@/components/ui/skeleton";

export function ScanStatus() {
  return (
    <div className="space-y-4" role="status">
      <p className="text-sm font-medium">Scanning folders</p>
      <p className="text-sm text-muted-foreground">
        Looking for supported video files in every subfolder.
      </p>
      <div className="space-y-2">
        <Skeleton className="h-4 w-2/3" />
        <Skeleton className="h-4 w-1/2" />
        <Skeleton className="h-4 w-3/5" />
      </div>
    </div>
  );
}
