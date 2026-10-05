import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";

interface CancelProcessingDialogProps {
  disabled?: boolean;
  cancelling?: boolean;
  onConfirm: () => void;
}

export function CancelProcessingDialog({
  disabled,
  cancelling,
  onConfirm,
}: CancelProcessingDialogProps) {
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button type="button" variant="outline" disabled={disabled || cancelling}>
          {cancelling ? "Cancelling" : "Cancel processing"}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Cancel batch processing?</AlertDialogTitle>
          <AlertDialogDescription>
            No new videos will start. The current video finishes safely, completed PDFs are kept,
            and the rest are left unprocessed.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Keep processing</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>Cancel processing</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
