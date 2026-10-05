import { FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import type { ExistingOutputPolicy, OutputDestination } from "@/lib/batch/types";
import { PolicySelector } from "./policy-selector";

interface OutputSettingsProps {
  inputRoot: string;
  newFolder: string;
  destination: OutputDestination;
  policy: ExistingOutputPolicy;
  disabled?: boolean;
  onDestinationChange: (destination: OutputDestination) => void;
  onPolicyChange: (policy: ExistingOutputPolicy) => void;
  onSelectOutput: () => void;
}

export function OutputSettings({
  inputRoot,
  newFolder,
  destination,
  policy,
  disabled,
  onDestinationChange,
  onPolicyChange,
  onSelectOutput,
}: OutputSettingsProps) {
  return (
    <div className="space-y-4">
      <fieldset className="space-y-2" disabled={disabled}>
        <legend className="text-sm font-medium">Where the PDFs go</legend>
        <RadioGroup
          value={destination}
          onValueChange={(value) => onDestinationChange(value as OutputDestination)}
        >
          <div className="flex items-center gap-2">
            <RadioGroupItem value="new-folder" id="destination-new-folder" />
            <Label htmlFor="destination-new-folder" className="font-normal">
              Into a new folder
            </Label>
          </div>
          <div className="flex items-center gap-2">
            <RadioGroupItem value="alongside" id="destination-alongside" />
            <Label htmlFor="destination-alongside" className="font-normal">
              Into the folder the videos came from
            </Label>
          </div>
        </RadioGroup>
      </fieldset>

      {destination === "new-folder" ? (
        <div className="space-y-2">
          <p className="text-sm font-medium">Output folder</p>
          <div className="flex flex-wrap items-center gap-3">
            <Button type="button" variant="outline" onClick={onSelectOutput} disabled={disabled}>
              <FolderOpen aria-hidden="true" />
              Change
            </Button>
            <p
              className="min-w-0 flex-1 truncate rounded-md border bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
              title={newFolder || undefined}
            >
              {newFolder || "No output folder selected"}
            </p>
          </div>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          Each PDF is written next to its video, in{" "}
          <span className="break-all font-medium text-foreground">{inputRoot}</span>.
        </p>
      )}

      <p className="text-xs text-muted-foreground">
        Directory structure is preserved. Source videos are never modified.
      </p>

      <PolicySelector policy={policy} disabled={disabled} onPolicyChange={onPolicyChange} />
    </div>
  );
}
