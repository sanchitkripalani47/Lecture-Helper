import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { AlertTriangle, Info, LockKeyhole } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { TooltipProvider } from "@/components/ui/tooltip";
import { basename } from "@/lib/batch/formatters";
import {
  createBatchClient,
  isPreviewMode,
  parentOf,
  type BatchClient,
} from "@/lib/batch/tauri-client";
import type {
  BatchEvent,
  ExistingOutputPolicy,
  InputKind,
  OutputDestination,
} from "@/lib/batch/types";
import { initialWorkflowState, workflowReducer } from "@/lib/batch/workflow";
import { cn } from "@/lib/utils";
import { BatchSummaryView } from "./batch-summary";
import { CancelProcessingDialog } from "./cancel-processing-dialog";
import { CurrentJobCard } from "./current-job-card";
import { FailedItemsDialog } from "./failed-items-dialog";
import { FolderPicker } from "./folder-picker";
import { OverallProgress } from "./overall-progress";
import { OutputSettings } from "./output-settings";
import { QueueSummary } from "./queue-summary";
import { QueueTree } from "./queue-tree";
import { ScanStatus } from "./scan-status";

const STEPS = ["Select videos", "Review queue", "Process", "Summary"] as const;

function stepIndex(stage: string): number {
  switch (stage) {
    case "setup":
      return 0;
    case "scanning":
      return 0;
    case "review":
      return 1;
    case "processing":
      return 2;
    case "summary":
      return 3;
    default:
      return 0;
  }
}

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Something went wrong while reading the folder.";
}

function announcementFor(event: BatchEvent, nameFor: (jobId: string) => string): string | null {
  switch (event.type) {
    case "jobStarted":
      return `${nameFor(event.jobId)} started`;
    case "jobCompleted":
      return `${nameFor(event.jobId)} completed`;
    case "jobFailed":
      return `${nameFor(event.jobId)} failed: ${event.error}`;
    case "jobSkipped":
      return `${nameFor(event.jobId)} skipped`;
    case "batchCancelled":
      return "Processing cancelled";
    case "batchCompleted":
      return "Batch complete";
    default:
      return null;
  }
}

export function BatchWorkflow({ client: injectedClient }: { client?: BatchClient }) {
  const client = useMemo(() => injectedClient ?? createBatchClient(), [injectedClient]);
  const [state, dispatch] = useReducer(workflowReducer, initialWorkflowState);
  const [policy, setPolicy] = useState<ExistingOutputPolicy>("skip");
  const [destination, setDestination] = useState<OutputDestination>("new-folder");
  const [announcement, setAnnouncement] = useState("");
  const [failuresOpen, setFailuresOpen] = useState(false);
  const jobsRef = useRef(state.jobs);
  jobsRef.current = state.jobs;

  // `state.outputRoot` remembers the chosen new folder. Writing alongside the
  // videos means using the folder that holds them, which for a single selected
  // file is its parent directory rather than the file itself.
  const alongsideFolder =
    state.inputKind === "file" ? parentOf(state.inputPath) : state.inputPath;
  const outputRoot = destination === "alongside" ? alongsideFolder : state.outputRoot;

  const options = useMemo(
    () => ({
      inputRoot: state.inputPath,
      outputRoot,
      existingOutputPolicy: policy,
    }),
    [state.inputPath, outputRoot, policy],
  );

  const nameFor = useCallback((jobId: string) => {
    const job = jobsRef.current.find((candidate) => candidate.id === jobId);
    return job ? basename(job.relativePath) : "Video";
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    void client
      .onBatchEvent((event) => {
        dispatch({ type: "batchEvent", event });
        const message = announcementFor(event, nameFor);
        if (message) setAnnouncement(message);
      })
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [client, nameFor]);

  const selectInput = useCallback(
    async (kind: InputKind, chooser: () => Promise<string | null>) => {
      try {
        const inputPath = await chooser();
        if (!inputPath) return;
        dispatch({
          type: "setInput",
          inputPath,
          inputKind: kind,
          outputRoot: client.defaultOutputRoot(inputPath, kind),
        });
      } catch (error) {
        dispatch({ type: "setError", error: describeError(error) });
      }
    },
    [client],
  );

  const handleSelectFolder = useCallback(
    () => selectInput("folder", () => client.selectInputFolder()),
    [client, selectInput],
  );

  const handleSelectFile = useCallback(
    () => selectInput("file", () => client.selectInputFile()),
    [client, selectInput],
  );

  const handleSelectOutput = useCallback(async () => {
    try {
      const outputRoot = await client.selectOutputFolder(
        state.outputRoot || client.defaultOutputRoot(state.inputPath, state.inputKind),
      );
      if (outputRoot) dispatch({ type: "setOutputRoot", outputRoot });
    } catch (error) {
      dispatch({ type: "setError", error: describeError(error) });
    }
  }, [client, state.inputKind, state.inputPath, state.outputRoot]);

  const handleScan = useCallback(async () => {
    dispatch({ type: "scanStarted" });
    setAnnouncement(state.inputKind === "file" ? "Reading the video" : "Scanning folders");
    try {
      const result = await client.discoverJobs(options);
      if (result.kind === "ready") {
        dispatch({ type: "scanReady", jobs: result.jobs });
        setAnnouncement(`Found ${result.jobs.length} videos`);
      } else {
        dispatch({ type: "scanFailed", error: "No supported videos found in this folder." });
        setAnnouncement("No supported videos found");
      }
    } catch (error) {
      dispatch({ type: "scanFailed", error: describeError(error) });
    }
  }, [client, options, state.inputKind]);

  const handleStart = useCallback(async () => {
    dispatch({ type: "processingStarted" });
    setAnnouncement("Processing started");
    try {
      await client.startBatch(options);
    } catch (error) {
      dispatch({ type: "batchFailed", error: describeError(error) });
    }
  }, [client, options]);

  const handleCancel = useCallback(async () => {
    dispatch({ type: "cancelRequested" });
    setAnnouncement("Cancelling after the current video");
    try {
      await client.cancelBatch();
    } catch (error) {
      dispatch({ type: "setError", error: describeError(error) });
    }
  }, [client]);

  const handleRestart = useCallback(() => dispatch({ type: "reset" }), []);
  // Whether the app is running in a plain browser rather than the desktop shell.
  const previewMode = isPreviewMode();
  const currentJob = state.jobs.find((job) => job.id === state.currentJobId) ?? null;
  const failedJobs = state.jobs.filter((job) => job.status === "failed");
  const activeStep = stepIndex(state.stage);

  return (
    <TooltipProvider delayDuration={200}>
      <div className="mx-auto flex min-h-screen w-full max-w-2xl flex-col gap-6 px-6 py-12">
        <header className="space-y-2">
          <h1 className="text-xl font-semibold tracking-tight">Lecture Helper</h1>
          <p className="flex items-center gap-1.5 text-sm text-muted-foreground">
            <LockKeyhole aria-hidden="true" className="h-3.5 w-3.5" />
            Processed locally on this device. Videos are never uploaded.
          </p>
        </header>

        <nav aria-label="Progress">
          <ol className="flex flex-wrap gap-x-4 gap-y-1 text-xs">
            {STEPS.map((label, index) => (
              <li
                key={label}
                aria-current={index === activeStep ? "step" : undefined}
                className={cn(
                  "flex items-center gap-1.5",
                  index === activeStep ? "font-medium text-foreground" : "text-muted-foreground",
                )}
              >
                <span
                  aria-hidden="true"
                  className={cn(
                    "flex h-5 w-5 items-center justify-center rounded-full border text-[11px]",
                    index === activeStep && "border-primary bg-primary text-primary-foreground",
                    index < activeStep && "border-primary/40 text-primary",
                  )}
                >
                  {index + 1}
                </span>
                {label}
              </li>
            ))}
          </ol>
        </nav>

        <div aria-live="polite" className="sr-only">
          {announcement}
        </div>

        <Card>
          <CardHeader>
            <CardTitle>{STEPS[activeStep]}</CardTitle>
            <CardDescription>{descriptions[state.stage]}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {state.error ? (
              <Alert variant="destructive">
                <AlertTriangle aria-hidden="true" />
                <AlertTitle>Could not continue</AlertTitle>
                <AlertDescription>{state.error}</AlertDescription>
              </Alert>
            ) : null}

            {state.stage === "setup" ? (
              <div className="space-y-6">
                {previewMode ? (
                  <Alert>
                    <Info aria-hidden="true" />
                    <AlertTitle>Browser preview</AlertTitle>
                    <AlertDescription>
                      Native folder selection is only available in the desktop app. Run{" "}
                      <code className="rounded bg-muted px-1 py-0.5 text-xs">
                        npm run tauri dev
                      </code>{" "}
                      for the real picker; this preview uses a sample queue.
                    </AlertDescription>
                  </Alert>
                ) : null}
                <FolderPicker
                  inputPath={state.inputPath}
                  inputKind={state.inputKind}
                  onSelectFolder={handleSelectFolder}
                  onSelectFile={handleSelectFile}
                />
                {state.inputPath ? (
                  <OutputSettings
                    inputRoot={alongsideFolder}
                    newFolder={state.outputRoot}
                    destination={destination}
                    policy={policy}
                    onDestinationChange={setDestination}
                    onPolicyChange={setPolicy}
                    onSelectOutput={handleSelectOutput}
                  />
                ) : null}
                <div className="flex flex-wrap gap-3">
                  <Button type="button" onClick={handleScan} disabled={!state.inputPath}>
                    {state.inputKind === "file" ? "Continue" : "Scan folder"}
                  </Button>
                </div>
              </div>
            ) : null}

            {state.stage === "scanning" ? <ScanStatus /> : null}

            {state.stage === "review" ? (
              <QueueSummary
                state={state}
                policy={policy}
                outputRoot={outputRoot}
                onPolicyChange={setPolicy}
                onStart={handleStart}
                onBack={() => dispatch({ type: "backToSetup" })}
              />
            ) : null}

            {state.stage === "processing" ? (
              <div className="space-y-6">
                <OverallProgress state={state} />
                <CurrentJobCard job={currentJob} />
                <QueueTree jobs={state.jobs} />
                <div className="flex flex-wrap gap-3">
                  <CancelProcessingDialog cancelling={state.cancelling} onConfirm={handleCancel} />
                </div>
              </div>
            ) : null}

            {state.stage === "summary" ? (
              <BatchSummaryView
                state={state}
                onRestart={handleRestart}
                onViewFailures={() => setFailuresOpen(true)}
              />
            ) : null}
          </CardContent>
        </Card>
      </div>

      <FailedItemsDialog jobs={failedJobs} open={failuresOpen} onOpenChange={setFailuresOpen} />
    </TooltipProvider>
  );
}

const descriptions: Record<string, string> = {
  setup: "Choose a folder of lecture videos and where the PDFs should go.",
  scanning: "Discovering supported videos in every subfolder.",
  review: "Check the queue before processing starts.",
  processing: "Videos are processed one at a time.",
  summary: "Review the batch results.",
};
