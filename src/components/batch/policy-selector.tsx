import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import type { ExistingOutputPolicy } from "@/lib/batch/types";

const OPTIONS: { value: ExistingOutputPolicy; id: string; label: string }[] = [
  { value: "skip", id: "policy-skip", label: "Skip existing" },
  { value: "replace", id: "policy-replace", label: "Replace existing" },
];

interface PolicySelectorProps {
  policy: ExistingOutputPolicy;
  disabled?: boolean;
  onPolicyChange: (policy: ExistingOutputPolicy) => void;
}

/** Shared between setup and queue review so the choice is always adjustable. */
export function PolicySelector({ policy, disabled, onPolicyChange }: PolicySelectorProps) {
  return (
    <fieldset className="space-y-2" disabled={disabled}>
      <legend className="text-sm font-medium">When output already exists</legend>
      <RadioGroup
        value={policy}
        onValueChange={(value) => onPolicyChange(value as ExistingOutputPolicy)}
      >
        {OPTIONS.map((option) => (
          <div key={option.value} className="flex items-center gap-2">
            <RadioGroupItem value={option.value} id={option.id} />
            <Label htmlFor={option.id} className="font-normal">
              {option.label}
            </Label>
          </div>
        ))}
      </RadioGroup>
    </fieldset>
  );
}
