import { FileVideo, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { InputKind } from "@/lib/batch/types";

interface FolderPickerProps {
  inputPath: string;
  inputKind: InputKind;
  disabled?: boolean;
  onSelectFolder: () => void;
  onSelectFile: () => void;
}

export function FolderPicker({
  inputPath,
  inputKind,
  disabled,
  onSelectFolder,
  onSelectFile,
}: FolderPickerProps) {
  return (
    <div className="space-y-2">
      <p className="text-sm font-medium">Lecture videos</p>
      <div className="flex flex-wrap items-center gap-3">
        <Button type="button" variant="outline" onClick={onSelectFolder} disabled={disabled}>
          <FolderOpen aria-hidden="true" />
          Choose folder
        </Button>
        <Button type="button" variant="outline" onClick={onSelectFile} disabled={disabled}>
          <FileVideo aria-hidden="true" />
          Choose video
        </Button>
      </div>
      <p
        className="truncate rounded-md border bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
        title={inputPath || undefined}
      >
        {inputPath || "No folder or video selected"}
      </p>
      <p className="text-xs text-muted-foreground">
        {inputKind === "file"
          ? "One video selected."
          : "Every supported video in the folder and its subfolders."}{" "}
        Supported formats: .mp4, .mov, .mkv, .avi, .webm
      </p>
    </div>
  );
}
