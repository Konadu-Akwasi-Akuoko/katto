import { convertFileSrc } from "@tauri-apps/api/core";
import type { Ref } from "react";
import { useImperativeHandle, useRef } from "react";

export type VideoPaneHandle = { seek: (seconds: number) => void };

/**
 * Plain `<video controls>` over the asset protocol — media bytes never cross
 * invoke. No custom chrome this phase; the handle exposes click-to-seek.
 */
export function VideoPane({
	sourcePath,
	ref,
}: {
	sourcePath: string;
	ref?: Ref<VideoPaneHandle>;
}) {
	const videoRef = useRef<HTMLVideoElement>(null);

	useImperativeHandle(ref, () => ({
		seek: (seconds: number) => {
			const video = videoRef.current;
			if (!video) return;
			video.currentTime = seconds;
		},
	}));

	return (
		// biome-ignore lint/a11y/useMediaCaption: source footage has no caption track yet
		<video
			ref={videoRef}
			src={convertFileSrc(sourcePath)}
			controls
			className="max-h-full w-full bg-bg"
		/>
	);
}
