import * as React from "react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 480 };

const panel: React.CSSProperties = {
  paddingTop: 14,
  fontSize: 13,
  lineHeight: 1.6,
  color: "var(--fg-muted)",
};

const list: React.CSSProperties = {
  margin: "8px 0 0",
  paddingLeft: 18,
  display: "flex",
  flexDirection: "column",
  gap: 4,
};

/** The clip inspector: Timeline / Transcript / Assets, with real panel content. */
export const ClipInspector = () => (
  <div style={frame}>
    <Tabs defaultValue="transcript">
      <TabsList>
        <TabsTrigger value="timeline">Timeline</TabsTrigger>
        <TabsTrigger value="transcript">Transcript</TabsTrigger>
        <TabsTrigger value="assets">Assets</TabsTrigger>
      </TabsList>
      <TabsContent value="timeline">
        <div style={panel}>
          18 clips on the rough cut, 12:41 total. Filler and dead air are flagged
          but never removed automatically — you make the final cut.
        </div>
      </TabsContent>
      <TabsContent value="transcript">
        <div style={panel}>
          Whisper transcript, aligned to the timeline. Click any line to jump the
          playhead.
          <ul style={list}>
            <li>00:04 — "Alright, so today we're rebuilding the ingest flow…"</li>
            <li>00:11 — "…and I want to show you the SD-card detection."</li>
            <li>00:19 — "This part tripped me up last time."</li>
          </ul>
        </div>
      </TabsContent>
      <TabsContent value="assets">
        <div style={panel}>
          B-roll, screen recordings, and thumbnails linked to this project. 3
          thumbnail concepts generated, none selected yet.
        </div>
      </TabsContent>
    </Tabs>
  </div>
);

/** Line variant used for a lighter, underline-style tab bar. */
export const LineVariant = () => (
  <div style={frame}>
    <Tabs defaultValue="planning">
      <TabsList variant="line">
        <TabsTrigger value="planning">Planning</TabsTrigger>
        <TabsTrigger value="ingest">Ingest</TabsTrigger>
        <TabsTrigger value="export">Export</TabsTrigger>
      </TabsList>
      <TabsContent value="planning">
        <div style={panel}>
          Outline, hooks, and shot list for the next upload. Nothing is scored —
          the AI suggests, you decide.
        </div>
      </TabsContent>
      <TabsContent value="ingest">
        <div style={panel}>Waiting on 2 SD cards to finish copying.</div>
      </TabsContent>
      <TabsContent value="export">
        <div style={panel}>Last export: draft-v3 · 1080p ProRes · 2 hours ago.</div>
      </TabsContent>
    </Tabs>
  </div>
);
