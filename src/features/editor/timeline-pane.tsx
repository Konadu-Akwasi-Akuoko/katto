import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { snapEdge } from "@/features/editor/model/snap";
import type {
	HitTarget,
	Viewport,
} from "@/features/editor/model/timeline-geometry";
import {
	hitTest,
	pxToTime,
	regionRects,
	rulerTicks,
	thumbSlots,
	timeToPx,
	zoomAround,
} from "@/features/editor/model/timeline-geometry";
import type { TokenSpan } from "@/features/editor/model/tokens";
import type { Range } from "@/features/editor/model/wire";
import type { DragTarget } from "@/features/editor/store/editor-store";
import { keyToDragTarget } from "@/features/editor/store/editor-store";
import type { Rational } from "@/lib/ipc/editor";

const RULER_HEIGHT = 20;
const TRACK_HEIGHT = 64;
const MIN_MARQUEE_SECONDS = 0.05;

/** Token colors resolved from the live CSS custom properties. */
function themeColors(el: HTMLElement) {
	const style = getComputedStyle(el);
	const token = (name: string, fallback: string) =>
		style.getPropertyValue(name).trim() || fallback;
	return {
		bg: token("--bg", "#111"),
		fg: token("--fg", "#eee"),
		fgFaint: token("--fg-faint", "#888"),
		warn: token("--warn", "#c90"),
		ember: token("--ember", "#d40"),
		hairline: token("--hairline", "#333"),
	};
}

/** Offscreen 8px 45°-hatch tile in the given stroke color. */
function hatchTile(stroke: string): HTMLCanvasElement {
	const tile = document.createElement("canvas");
	tile.width = 8;
	tile.height = 8;
	const ctx = tile.getContext("2d");
	if (ctx) {
		ctx.strokeStyle = stroke;
		ctx.globalAlpha = 0.35;
		ctx.lineWidth = 1;
		ctx.beginPath();
		ctx.moveTo(-2, 10);
		ctx.lineTo(10, -2);
		ctx.moveTo(-2, 6);
		ctx.lineTo(6, -2);
		ctx.stroke();
	}
	return tile;
}

type PointerDrag =
	| { mode: "edge"; target: DragTarget; edge: "start" | "end" }
	| { mode: "marquee"; startT: number; endT: number }
	| { mode: "maybe-seek"; t: number };

/**
 * Canvas timeline: mono ruler, filmstrip track with cut regions rendered as
 * dimmed/hatched absence, playhead, edge drags with token snap (Option =
 * free), marquee + release for manual cuts. Deliberately thin — all math in
 * timeline-geometry.
 */
export function TimelinePane({
	duration,
	fps,
	tokens,
	ranges,
	discretionaryUnapplied,
	selectedKey,
	currentTime,
	viewport,
	bundlePath,
	thumbsVersion,
	onThumbsMissing,
	onViewportChange,
	onSeek,
	onSelect,
	onDragBegin,
	onDrag,
	onDragEnd,
	onMarqueeCut,
}: {
	duration: number;
	fps: Rational;
	tokens: TokenSpan[];
	ranges: Array<Range & { key: string }>;
	discretionaryUnapplied: Array<Range & { key: string }>;
	selectedKey: string | null;
	currentTime: number;
	viewport: Viewport;
	bundlePath: string;
	thumbsVersion: number;
	onThumbsMissing(): void;
	onViewportChange(vp: Viewport): void;
	onSeek(t: number): void;
	onSelect(key: string | null): void;
	onDragBegin(): void;
	onDrag(target: DragTarget, edge: "start" | "end", t: number): void;
	onDragEnd(commit: boolean): void;
	onMarqueeCut(range: Range): void;
}) {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const imagesRef = useRef<Map<number, HTMLImageElement>>(new Map());
	const dragRef = useRef<PointerDrag | null>(null);
	const [marquee, setMarquee] = useState<Range | null>(null);
	const [thumbsReady, setThumbsReady] = useState(false);
	const missingReported = useRef(false);
	// Filmstrip images decode async; a loaded frame re-renders so the draw
	// effect paints it (without this, frames appear only on the next redraw).
	const [, bumpRedraw] = useReducer((x: number) => x + 1, 0);

	// Probe the first thumbnail: present -> filmstrip on; absent -> ask once.
	// biome-ignore lint/correctness/useExhaustiveDependencies: thumbsVersion re-runs the probe after the regeneration job lands
	useEffect(() => {
		imagesRef.current.clear();
		setThumbsReady(false);
		const probe = new Image();
		probe.onload = () => setThumbsReady(true);
		probe.onerror = () => {
			if (!missingReported.current) {
				missingReported.current = true;
				onThumbsMissing();
			}
		};
		probe.src = convertFileSrc(`${bundlePath}/thumbs/00001.jpg`);
	}, [bundlePath, thumbsVersion, onThumbsMissing]);

	const draw = useCallback(() => {
		const canvas = canvasRef.current;
		const container = containerRef.current;
		if (!canvas || !container) return;
		const dpr = window.devicePixelRatio || 1;
		const width = viewport.widthPx;
		const height = RULER_HEIGHT + TRACK_HEIGHT;
		if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
			canvas.width = width * dpr;
			canvas.height = height * dpr;
		}
		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		const colors = themeColors(container);
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.clearRect(0, 0, width, height);

		// Filmstrip.
		const trackTop = RULER_HEIGHT;
		ctx.fillStyle = colors.bg;
		ctx.fillRect(0, trackTop, width, TRACK_HEIGHT);
		if (thumbsReady) {
			ctx.globalAlpha = 0.85;
			for (const slot of thumbSlots(viewport, duration)) {
				let img = imagesRef.current.get(slot.index);
				if (img === undefined) {
					img = new Image();
					img.onload = () => bumpRedraw();
					img.src = convertFileSrc(
						`${bundlePath}/thumbs/${String(slot.index).padStart(5, "0")}.jpg`,
					);
					imagesRef.current.set(slot.index, img);
				}
				if (img.complete && img.naturalWidth > 0) {
					ctx.drawImage(img, slot.xPx, trackTop, slot.wPx, TRACK_HEIGHT);
				}
			}
			ctx.globalAlpha = 1;
		}

		// Cut regions: absence (dimmed + hatch), discretionary dotted outline.
		const rects = regionRects(ranges, discretionaryUnapplied, viewport);
		const hatch = ctx.createPattern(hatchTile(colors.fgFaint), "repeat");
		for (const rect of rects) {
			const x = rect.startPx;
			const w = rect.endPx - rect.startPx;
			if (rect.kind === "cut") {
				ctx.globalAlpha = 0.75;
				ctx.fillStyle = colors.bg;
				ctx.fillRect(x, trackTop, w, TRACK_HEIGHT);
				ctx.globalAlpha = 1;
				if (hatch) {
					ctx.fillStyle = hatch;
					ctx.fillRect(x, trackTop, w, TRACK_HEIGHT);
				}
			} else {
				ctx.save();
				ctx.strokeStyle = colors.warn;
				ctx.setLineDash([3, 3]);
				ctx.strokeRect(x + 0.5, trackTop + 1.5, w - 1, TRACK_HEIGHT - 3);
				ctx.restore();
			}
			if (rect.key === selectedKey) {
				ctx.strokeStyle = colors.ember;
				ctx.lineWidth = 1;
				ctx.strokeRect(x + 0.5, trackTop + 0.5, w - 1, TRACK_HEIGHT - 1);
			}
		}

		// Marquee preview.
		if (marquee) {
			const x = timeToPx(Math.min(marquee.start, marquee.end), viewport);
			const w = Math.abs(marquee.end - marquee.start) * viewport.pxPerSec;
			ctx.globalAlpha = 0.25;
			ctx.fillStyle = colors.ember;
			ctx.fillRect(x, trackTop, w, TRACK_HEIGHT);
			ctx.globalAlpha = 1;
		}

		// Ruler.
		ctx.strokeStyle = colors.hairline;
		ctx.beginPath();
		ctx.moveTo(0, RULER_HEIGHT - 0.5);
		ctx.lineTo(width, RULER_HEIGHT - 0.5);
		ctx.stroke();
		ctx.font = "10px ui-monospace, monospace";
		ctx.fillStyle = colors.fgFaint;
		ctx.textBaseline = "top";
		for (const tick of rulerTicks(viewport, duration)) {
			const x = Math.round(timeToPx(tick.t, viewport)) + 0.5;
			ctx.strokeStyle = colors.hairline;
			ctx.beginPath();
			ctx.moveTo(x, tick.major ? 4 : 12);
			ctx.lineTo(x, RULER_HEIGHT);
			ctx.stroke();
			if (tick.major) {
				const m = Math.floor(tick.t / 60);
				const s = Math.round(tick.t - m * 60);
				ctx.fillText(`${m}:${String(s).padStart(2, "0")}`, x + 3, 3);
			}
		}

		// Playhead: 1px line + top handle.
		const px = timeToPx(currentTime, viewport);
		if (px >= 0 && px <= width) {
			ctx.strokeStyle = colors.fg;
			ctx.beginPath();
			ctx.moveTo(Math.round(px) + 0.5, 0);
			ctx.lineTo(Math.round(px) + 0.5, height);
			ctx.stroke();
			ctx.fillStyle = colors.fg;
			ctx.fillRect(Math.round(px) - 1, 0, 3, 8);
		}
	}, [
		viewport,
		duration,
		ranges,
		discretionaryUnapplied,
		selectedKey,
		currentTime,
		marquee,
		thumbsReady,
		bundlePath,
	]);

	useEffect(() => {
		draw();
	});

	// Container width -> viewport width (HiDPI redraw included).
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;
		const observer = new ResizeObserver(() => {
			const width = container.clientWidth;
			if (width > 0 && width !== viewport.widthPx) {
				onViewportChange({ ...viewport, widthPx: width });
			}
		});
		observer.observe(container);
		return () => observer.disconnect();
	}, [viewport, onViewportChange]);

	const localX = (e: React.PointerEvent): number => {
		const rect = canvasRef.current?.getBoundingClientRect();
		return e.clientX - (rect?.left ?? 0);
	};

	const handlePointerDown = (e: React.PointerEvent) => {
		const x = localX(e);
		const rects = regionRects(ranges, discretionaryUnapplied, viewport);
		const hit: HitTarget = hitTest(x, rects, viewport);
		canvasRef.current?.setPointerCapture(e.pointerId);
		if (hit.kind === "edge") {
			const target = keyToDragTarget(hit.key);
			if (target) {
				dragRef.current = { mode: "edge", target, edge: hit.edge };
				onDragBegin();
				return;
			}
		}
		if (hit.kind === "region") {
			onSelect(hit.key);
			dragRef.current = null;
			return;
		}
		if (hit.kind === "empty") {
			dragRef.current = { mode: "maybe-seek", t: hit.t };
		}
	};

	const handlePointerMove = (e: React.PointerEvent) => {
		const drag = dragRef.current;
		if (!drag) return;
		const t = pxToTime(localX(e), viewport);
		if (drag.mode === "edge") {
			onDrag(drag.target, drag.edge, snapEdge(t, tokens, fps, e.altKey));
			return;
		}
		if (drag.mode === "maybe-seek") {
			dragRef.current = { mode: "marquee", startT: drag.t, endT: t };
			setMarquee({ start: drag.t, end: t });
			return;
		}
		dragRef.current = { ...drag, endT: t };
		setMarquee({ start: drag.startT, end: t });
	};

	const handlePointerUp = () => {
		const drag = dragRef.current;
		dragRef.current = null;
		if (!drag) return;
		if (drag.mode === "edge") {
			onDragEnd(true);
			return;
		}
		if (drag.mode === "maybe-seek") {
			onSeek(Math.max(0, drag.t));
			onSelect(null);
			return;
		}
		setMarquee(null);
		const start = Math.min(drag.startT, drag.endT);
		const end = Math.max(drag.startT, drag.endT);
		if (end - start >= MIN_MARQUEE_SECONDS) {
			onMarqueeCut({ start, end });
		}
	};

	const handleWheel = (e: React.WheelEvent) => {
		if (e.shiftKey) {
			const factor = e.deltaY < 0 ? 1.2 : 1 / 1.2;
			onViewportChange(
				zoomAround(
					viewport,
					localX(e as unknown as React.PointerEvent),
					factor,
					duration,
				),
			);
			return;
		}
		const delta = (e.deltaX || e.deltaY) / viewport.pxPerSec;
		onViewportChange({
			...viewport,
			scrollSec: Math.max(0, viewport.scrollSec + delta),
		});
	};

	return (
		<div ref={containerRef} className="relative w-full">
			<canvas
				ref={canvasRef}
				className="block w-full"
				style={{ height: RULER_HEIGHT + TRACK_HEIGHT }}
				onPointerDown={handlePointerDown}
				onPointerMove={handlePointerMove}
				onPointerUp={handlePointerUp}
				onPointerCancel={() => {
					if (dragRef.current?.mode === "edge") onDragEnd(false);
					dragRef.current = null;
					setMarquee(null);
				}}
				onWheel={handleWheel}
			/>
			{!thumbsReady && (
				<span className="pointer-events-none absolute right-2 top-6 text-xs text-fg-faint">
					generating thumbnails…
				</span>
			)}
		</div>
	);
}
