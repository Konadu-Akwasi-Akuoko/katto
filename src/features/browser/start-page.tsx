import {
	CameraIcon,
	DribbbleLogoIcon,
	type Icon,
	ImageSquareIcon,
	PaletteIcon,
	PinterestLogoIcon,
	StackIcon,
	WaveformIcon,
	YoutubeLogoIcon,
} from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import texture from "@/assets/start/texture.webp";
import { activeAssetProject, browserKeys } from "@/lib/ipc/browser";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import { toNavigable } from "./model/address";

const TILES: { label: string; url: string; mark: Icon }[] = [
	{
		label: "Envato Elements",
		url: "https://elements.envato.com/",
		mark: StackIcon,
	},
	{ label: "Dribbble", url: "https://dribbble.com/", mark: DribbbleLogoIcon },
	{
		label: "Pinterest",
		url: "https://www.pinterest.com/",
		mark: PinterestLogoIcon,
	},
	{
		label: "TestMyThumbnails",
		url: "https://www.testmythumbnails.com/",
		mark: ImageSquareIcon,
	},
	{
		label: "YouTube Studio",
		url: "https://studio.youtube.com/",
		mark: YoutubeLogoIcon,
	},
	{ label: "Unsplash", url: "https://unsplash.com/", mark: CameraIcon },
	{ label: "Freesound", url: "https://freesound.org/", mark: WaveformIcon },
	{ label: "Coolors", url: "https://coolors.co/", mark: PaletteIcon },
];

function greeting(hour: number): string {
	if (hour < 12) return "Morning";
	if (hour < 17) return "Afternoon";
	return "Evening";
}

/** Ticks on the minute rather than the second — nothing here shows seconds. */
function useNow(): Date {
	const [now, setNow] = useState(() => new Date());
	useEffect(() => {
		const id = setInterval(() => setNow(new Date()), 30_000);
		return () => clearInterval(id);
	}, []);
	return now;
}

function isoDay(date: Date): string {
	const month = `${date.getMonth() + 1}`.padStart(2, "0");
	const day = `${date.getDate()}`.padStart(2, "0");
	return `${date.getFullYear()}-${month}-${day}`;
}

function Fact({ label, value }: { label: string; value: string }) {
	return (
		<div className="flex min-w-0 flex-col gap-0.5">
			<span className="text-[11px] text-fg-faint">{label}</span>
			<span className="truncate text-xs text-fg-muted">{value}</span>
		</div>
	);
}

/**
 * What a tab without a URL shows. A dark, warm textured surface inside the light
 * app — a workshop wall rather than a second dashboard. The `dark` class makes
 * every token resolve to its dark-theme value, so nothing here hardcodes a
 * colour to sit on the texture.
 *
 * The facts row is deliberately studio state (what files where, what ships
 * today) rather than the weather-and-stocks widgets a generic new tab shows.
 */
export function StartPage({
	onNavigate,
}: {
	onNavigate: (url: string) => void;
}) {
	const [draft, setDraft] = useState("");
	const now = useNow();
	const today = isoDay(now);

	const target = useQuery({
		queryKey: browserKeys.activeProject,
		queryFn: activeAssetProject,
	});
	const projects = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	const all = projects.data ?? [];
	const shooting = all.filter((p) => p.shoot_date === today);
	const publishing = all.filter((p) => p.publish_date === today);

	function submit() {
		const next = toNavigable(draft);
		if (next !== null) onNavigate(next);
	}

	return (
		<div className="dark relative h-full overflow-hidden bg-bg">
			<div
				aria-hidden
				className="absolute inset-0 bg-center bg-cover opacity-60"
				style={{ backgroundImage: `url(${texture})` }}
			/>
			{/* scrim strongest at the edges: the texture reads behind the block
			    without ever fighting the text sitting on it */}
			<div
				aria-hidden
				className="absolute inset-0 bg-gradient-to-b from-bg/95 via-bg/70 to-bg/95"
			/>

			<div className="relative flex h-full flex-col overflow-y-auto px-8">
				<div className="m-auto flex w-full max-w-2xl flex-col gap-6 py-10">
					<div className="flex flex-col items-center gap-1 text-center">
						<span className="text-xs text-fg-muted">
							{now.toLocaleDateString(undefined, {
								weekday: "long",
								day: "numeric",
								month: "long",
							})}
						</span>
						<span className="font-serif text-5xl text-fg tabular-nums">
							{now.toLocaleTimeString(undefined, {
								hour: "2-digit",
								minute: "2-digit",
								hour12: false,
							})}
						</span>
						<span className="text-sm text-fg-muted">
							{greeting(now.getHours())}. Downloads land in the active project's
							assets folder — never in ~/Downloads.
						</span>
					</div>

					<input
						type="text"
						aria-label="Search or enter address"
						placeholder="Search or enter an address"
						value={draft}
						onChange={(e) => setDraft(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") submit();
						}}
						spellCheck={false}
						autoCorrect="off"
						autoCapitalize="off"
						className="h-9 w-full min-w-0 cursor-text rounded-md border bg-surface/80 px-3 text-sm text-fg placeholder:text-fg-faint focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
					/>

					<div className="grid grid-cols-4 gap-1.5">
						{TILES.map(({ label, url, mark: Mark }) => (
							<button
								key={url}
								type="button"
								onClick={() => onNavigate(url)}
								className="flex h-9 items-center gap-2 rounded-md border border-transparent bg-surface/60 px-2.5 text-left text-xs text-fg-muted hover:border-border hover:bg-surface hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
							>
								<Mark className="size-4 shrink-0 text-fg-faint" />
								<span className="truncate">{label}</span>
							</button>
						))}
					</div>

					<div className="grid grid-cols-3 gap-4 border-t pt-4">
						<Fact label="Filing to" value={target.data ?? "no project"} />
						<Fact
							label="Shooting today"
							value={
								shooting.length === 0
									? "nothing"
									: shooting.map((p) => p.title).join(", ")
							}
						/>
						<Fact
							label="Publishing today"
							value={
								publishing.length === 0
									? "nothing"
									: publishing.map((p) => p.title).join(", ")
							}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
