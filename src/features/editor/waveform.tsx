import { useEffect, useRef, useState } from "react";
import WaveSurfer from "wavesurfer.js";
import RegionsPlugin from "wavesurfer.js/dist/plugins/regions.esm.js";

import type { Viewport } from "@/features/editor/model/timeline-geometry";
import type { Range } from "@/features/editor/model/wire";

const WAVEFORM_HEIGHT = 72;

/** Append an alpha channel to a computed CSS color (oklch/rgb functional). */
function withAlpha(color: string, alpha: number): string {
	const trimmed = color.trim();
	if (trimmed.endsWith(")")) {
		return `${trimmed.slice(0, -1)} / ${alpha})`;
	}
	return trimmed || `rgba(0, 0, 0, ${alpha})`;
}

type RegionLike = {
	id: string;
	start: number;
	end: number;
	updatingSide?: "start" | "end";
	setOptions(options: {
		start?: number;
		end?: number;
		color?: string;
		drag?: boolean;
		resize?: boolean;
	}): void;
	remove(): void;
};

/**
 * Display-only wavesurfer strip mirroring the effective cut regions. The
 * `<video>` owns playback: `ws.play()` is never called; `currentTime` drives
 * `setTime`, and the canvas timeline owns the viewport (one-way zoom/scroll
 * sync — wavesurfer's own scroll/zoom events are NOT fed back).
 */
export function Waveform({
	audioUrl,
	ranges,
	currentTime,
	viewport,
	selectedKey,
	onSeek,
	onDragBegin,
	onDrag,
	onDragEnd,
	onSelect,
}: {
	audioUrl: string;
	ranges: Array<Range & { key: string }>;
	currentTime: number;
	viewport: Viewport;
	selectedKey: string | null;
	onSeek(t: number): void;
	onDragBegin(): void;
	onDrag(key: string, edge: "start" | "end", t: number): void;
	onDragEnd(): void;
	onSelect(key: string | null): void;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const wsRef = useRef<WaveSurfer | null>(null);
	const regionsRef = useRef<InstanceType<typeof RegionsPlugin> | null>(null);
	const ownIds = useRef<Set<string>>(new Set());
	const regionMap = useRef<Map<string, RegionLike>>(new Map());
	const draggingRef = useRef(false);
	const [ready, setReady] = useState(false);

	// Latest handlers without re-creating wavesurfer.
	const handlers = useRef({ onSeek, onDragBegin, onDrag, onDragEnd, onSelect });
	handlers.current = { onSeek, onDragBegin, onDrag, onDragEnd, onSelect };

	// biome-ignore lint/correctness/useExhaustiveDependencies: viewport.pxPerSec only seeds minPxPerSec at create; the zoom effect below owns live sync
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;
		const style = getComputedStyle(container);
		const token = (name: string, fallback: string) =>
			style.getPropertyValue(name).trim() || fallback;

		const regions = RegionsPlugin.create();
		const ws = WaveSurfer.create({
			container,
			url: audioUrl,
			height: WAVEFORM_HEIGHT,
			waveColor: token("--fg-faint", "#888"),
			progressColor: token("--fg-muted", "#aaa"),
			cursorColor: token("--ember", "#d40"),
			barWidth: 2,
			barGap: 1,
			barRadius: 1,
			minPxPerSec: viewport.pxPerSec,
			normalize: true,
			interact: true,
			dragToSeek: false,
			autoCenter: false,
			plugins: [regions],
		});
		wsRef.current = ws;
		regionsRef.current = regions;

		ws.on("ready", () => setReady(true));
		ws.on("interaction", (newTime: number) => {
			handlers.current.onSeek(newTime);
		});

		const sideOf = (
			region: RegionLike,
			side?: "start" | "end",
		): "start" | "end" => side ?? region.updatingSide ?? "end";

		// Only region-update / region-updated exist (no -start/-end events);
		// updatingSide is set ONLY during user drags, so programmatic
		// setOptions never re-enters here.
		regions.on(
			"region-update",
			(region: RegionLike, side?: "start" | "end") => {
				if (region.updatingSide === undefined) return;
				const edge = sideOf(region, side);
				if (!draggingRef.current) {
					draggingRef.current = true;
					handlers.current.onDragBegin();
				}
				handlers.current.onDrag(
					region.id,
					edge,
					edge === "start" ? region.start : region.end,
				);
			},
		);
		regions.on(
			"region-updated",
			(region: RegionLike, side?: "start" | "end") => {
				if (!draggingRef.current) return;
				const edge = sideOf(region, side);
				handlers.current.onDrag(
					region.id,
					edge,
					edge === "start" ? region.start : region.end,
				);
				draggingRef.current = false;
				handlers.current.onDragEnd();
			},
		);
		regions.on("region-clicked", (region: RegionLike) => {
			handlers.current.onSelect(region.id);
		});

		return () => {
			setReady(false);
			regionMap.current.clear();
			ownIds.current.clear();
			wsRef.current = null;
			regionsRef.current = null;
			ws.destroy();
		};
		// audioUrl identifies the bundle; everything else syncs via effects.
	}, [audioUrl]);

	// Video owns playback: currentTime is display-only.
	useEffect(() => {
		const ws = wsRef.current;
		if (!ws || !ready) return;
		ws.setTime(currentTime);
	}, [currentTime, ready]);

	// One-way viewport sync from the canvas timeline.
	useEffect(() => {
		const ws = wsRef.current;
		if (!ws || !ready) return;
		ws.zoom(viewport.pxPerSec);
		ws.setScroll(viewport.scrollSec * viewport.pxPerSec);
	}, [viewport, ready]);

	// Region reconciliation — never before ready/non-zero duration (bounds
	// would clamp to 0 permanently).
	useEffect(() => {
		const ws = wsRef.current;
		const regions = regionsRef.current;
		const container = containerRef.current;
		if (!ws || !regions || !container || !ready || ws.getDuration() <= 0) {
			return;
		}
		const style = getComputedStyle(container);
		const bg = style.getPropertyValue("--bg").trim() || "#111";
		const ember = style.getPropertyValue("--ember").trim() || "#d40";
		const cutFill = withAlpha(bg, 0.55);
		const selectedFill = withAlpha(ember, 0.25);

		const wanted = new Map(ranges.map((r) => [r.key, r]));
		for (const [id, region] of regionMap.current) {
			if (!wanted.has(id)) {
				// Never tear down a region mid-drag: remove() kills wavesurfer's
				// drag subscriptions before region-updated fires, so commitDrag
				// would never run and the temporal store would stay paused. A
				// key can drop mid-drag (disc-to-manual conversion, inverted
				// drag); the post-commit reconcile pass sweeps it up.
				if (region.updatingSide !== undefined) continue;
				region.remove();
				regionMap.current.delete(id);
			}
		}
		for (const [key, range] of wanted) {
			const color = key === selectedKey ? selectedFill : cutFill;
			const existing = regionMap.current.get(key);
			if (existing) {
				// Leave a region alone mid-drag; programmatic setOptions never
				// trips the update loop (no updatingSide).
				if (existing.updatingSide === undefined) {
					existing.setOptions({
						start: range.start,
						end: range.end,
						color,
					});
				}
				continue;
			}
			ownIds.current.add(key);
			const region = regions.addRegion({
				id: key,
				start: range.start,
				end: range.end,
				color,
				drag: false,
				resize: true,
			}) as unknown as RegionLike;
			regionMap.current.set(key, region);
		}
	}, [ranges, selectedKey, ready]);

	return <div ref={containerRef} className="w-full" />;
}
