import * as React from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  Button,
} from "katto";

/** A confirmation dialog, shown open over its modal overlay. */
export const Open = () => (
  <Dialog open>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Discard this rough cut?</DialogTitle>
        <DialogDescription>
          draft-v3 has unsaved trims. Discarding drops the AI assembly and
          returns the clips to the ingest tray. This can't be undone.
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline">Keep editing</Button>
        <Button variant="destructive">Discard cut</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
);
