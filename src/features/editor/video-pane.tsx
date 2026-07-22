import { convertFileSrc } from "@tauri-apps/api/core";
import type { Ref } from "react";
import { useImperativeHandle, useRef } from "react";

export type VideoPaneHandle = {
	seek(seconds: number): void;
	getCurrentTime(): number;
	play(): void;
	pause(): void;
	isPaused(): boolean;
	setRate(rate: number): void;
	getRate(): number;
};

/**
 * Plain `<video>` over the asset protocol — media bytes never cross invoke.
 * The transport row owns playback chrome; the handle exposes the controls the
 * transport hook needs.
 */
export function VideoPane({
	sourcePath,
	onTimeUpdate,
	onPlayingChange,
	ref,
}: {
	sourcePath: string;
	onTimeUpdate?: (seconds: number) => void;
	onPlayingChange?: (playing: boolean) => void;
	ref?: Ref<VideoPaneHandle>;
}) {
	const videoRef = useRef<HTMLVideoElement>(null);

	useImperativeHandle(ref, () => ({
		seek: (seconds: number) => {
			const video = videoRef.current;
			if (!video) return;
			video.currentTime = seconds;
		},
		getCurrentTime: () => videoRef.current?.currentTime ?? 0,
		play: () => {
			void videoRef.current?.play();
		},
		pause: () => videoRef.current?.pause(),
		isPaused: () => videoRef.current?.paused ?? true,
		setRate: (rate: number) => {
			const video = videoRef.current;
			if (video) video.playbackRate = rate;
		},
		getRate: () => videoRef.current?.playbackRate ?? 1,
	}));

	return (
		// biome-ignore lint/a11y/useMediaCaption: source footage has no caption track yet
		<video
			ref={videoRef}
			src={convertFileSrc(sourcePath)}
			className="max-h-full w-full bg-bg"
			onTimeUpdate={(e) => onTimeUpdate?.(e.currentTarget.currentTime)}
			onPlay={() => onPlayingChange?.(true)}
			onPause={() => onPlayingChange?.(false)}
		/>
	);
}
