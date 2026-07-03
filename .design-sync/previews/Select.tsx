import * as React from "react";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectGroup,
  SelectLabel,
  SelectItem,
} from "katto";

const frame: React.CSSProperties = { padding: 24, display: "flex", gap: 14, flexWrap: "wrap" };

const formats = (
  <SelectContent>
    <SelectGroup>
      <SelectLabel>Resolution</SelectLabel>
      <SelectItem value="2160p">2160p · 4K</SelectItem>
      <SelectItem value="1080p">1080p · HD</SelectItem>
      <SelectItem value="720p">720p</SelectItem>
    </SelectGroup>
  </SelectContent>
);

/** Trigger with a chosen value, and an empty trigger showing its placeholder. */
export const Triggers = () => (
  <div style={frame}>
    <Select defaultValue="1080p">
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      {formats}
    </Select>
    <Select>
      <SelectTrigger>
        <SelectValue placeholder="Export format" />
      </SelectTrigger>
      {formats}
    </Select>
  </div>
);

/** Small size and a disabled trigger. */
export const SizesAndDisabled = () => (
  <div style={frame}>
    <Select defaultValue="prores">
      <SelectTrigger size="sm">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="prores">ProRes 422</SelectItem>
        <SelectItem value="h264">H.264</SelectItem>
      </SelectContent>
    </Select>
    <Select defaultValue="1080p" disabled>
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      {formats}
    </Select>
  </div>
);
