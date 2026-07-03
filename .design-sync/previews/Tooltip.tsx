import * as React from "react";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
  Button,
} from "katto";

/** A tooltip shown open above its trigger. */
export const Open = () => (
  <div style={{ padding: 64, display: "flex", justifyContent: "center" }}>
    <TooltipProvider>
      <Tooltip open>
        <TooltipTrigger asChild>
          <Button variant="outline">Render</Button>
        </TooltipTrigger>
        <TooltipContent side="top" sideOffset={6}>
          Renders the current timeline to ProRes 422
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  </div>
);
