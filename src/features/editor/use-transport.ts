import type { RefObject } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
	coalesceRanges,
	seekPastCut,
} from "@/features/editor/model/kept-ranges";
import type { Range } from "@/features/editor/model/wire";
import type { TransportAction } from "@/features/editor/transport";
import type { VideoPaneHandle } from "@/features/editor/video-pane";
import type { Rational } from "@/lib/ipc/bindings.gen";

/** J-shuttle: step back this many seconds per tick (≈1× reverse). */
const SHUTTLE_STEP_SECONDS = 0.25;
const SHUTTLE_TICK_MS = 250;

/**
 * Wires a `<video>` element to kept-only playback and JKL transport under
 * HTML5 constraints: K = pause, L = play (second L = 2×), J = reverse-ish
 * shuttle via a stepping interval, Space toggles, arrows frame-step.
 * Undo/redo/manual-cut actions are the caller's concern.
 */
export function useTransport(opts: {
	videoRef: RefObject<VideoPaneHandle | null>;
	getCutRanges(): Range[]; // effective ranges (kept-only ignores when showOriginal)
	fps: Rational;
}): {
	showOriginal: boolean;
	toggleOriginal(): void;
	currentTime: number;
	playing: boolean;
	rate: number;
	onTimeUpdate(t: number): void;
	onPlayingChange(playing: boolean): void;
	dispatch(a: TransportAction): void;
} {
	const { videoRef, getCutRanges, fps } = opts;
	const [showOriginal, setShowOriginal] = useState(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [playing, setPlaying] = useState(false);
	const [rate, setRateState] = useState(1);
	const shuttleTimer = useRef<ReturnType<typeof setInterval> | null>(null);

	const clearShuttle = useCallback(() => {
		if (shuttleTimer.current !== null) {
			clearInterval(shuttleTimer.current);
			shuttleTimer.current = null;
		}
	}, []);

	useEffect(() => clearShuttle, [clearShuttle]);

	const setRate = useCallback(
		(value: number) => {
			videoRef.current?.setRate(value);
			setRateState(value);
		},
		[videoRef],
	);

	const pause = useCallback(() => {
		videoRef.current?.pause();
		setPlaying(false);
	}, [videoRef]);

	const play = useCallback(() => {
		clearShuttle();
		videoRef.current?.play();
		setPlaying(true);
	}, [videoRef, clearShuttle]);

	const onTimeUpdate = useCallback(
		(t: number) => {
			setCurrentTime(t);
			if (showOriginal) return;
			const target = seekPastCut(t, coalesceRanges(getCutRanges()));
			if (target !== null) videoRef.current?.seek(target);
		},
		[showOriginal, getCutRanges, videoRef],
	);

	const dispatch = useCallback(
		(action: TransportAction) => {
			const video = videoRef.current;
			if (!video) return;
			switch (action.kind) {
				case "toggle-play": {
					clearShuttle();
					if (video.isPaused()) {
						setRate(1);
						play();
					} else {
						pause();
					}
					break;
				}
				case "stop": {
					clearShuttle();
					setRate(1);
					pause();
					break;
				}
				case "play-forward": {
					clearShuttle();
					if (!video.isPaused()) {
						setRate(2); // stacking cap
					} else {
						setRate(1);
						play();
					}
					break;
				}
				case "shuttle-back": {
					pause();
					if (shuttleTimer.current !== null) break;
					shuttleTimer.current = setInterval(() => {
						const now = videoRef.current?.getCurrentTime() ?? 0;
						videoRef.current?.seek(Math.max(0, now - SHUTTLE_STEP_SECONDS));
					}, SHUTTLE_TICK_MS);
					break;
				}
				case "step-frames": {
					clearShuttle();
					pause();
					const frame = fps.den / fps.num;
					const now = video.getCurrentTime();
					video.seek(Math.max(0, now + action.frames * frame));
					break;
				}
				case "toggle-original": {
					setShowOriginal((v) => !v);
					break;
				}
				default:
					break; // undo/redo/manual-cut are handled by the caller
			}
		},
		[videoRef, fps, clearShuttle, setRate, play, pause],
	);

	return {
		showOriginal,
		toggleOriginal: () => setShowOriginal((v) => !v),
		currentTime,
		playing,
		rate,
		onTimeUpdate,
		onPlayingChange: setPlaying,
		dispatch,
	};
}
