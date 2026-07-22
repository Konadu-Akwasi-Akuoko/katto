import { FileIcon, FilmSlateIcon } from "@phosphor-icons/react";

import { Checkbox } from "@/components/ui/checkbox";
import {
	allPathsIn,
	formatBytes,
	formatDuration,
} from "@/features/ingest/model/select";
import type { CardOffer } from "@/lib/ipc/ingest";

interface Props {
	offer: CardOffer;
	selected: ReadonlySet<string>;
	onToggle: (path: string, on: boolean) => void;
	onToggleGroup: (paths: string[], on: boolean) => void;
}

/**
 * Grouped card contents. Videos are pre-selected; sidecars (.XML/.THM) are
 * listed but deselected per the PRD — the group select-all covers videos only.
 */
export function ClipGroupList({
	offer,
	selected,
	onToggle,
	onToggleGroup,
}: Props) {
	return (
		<div className="flex flex-col gap-4">
			{offer.groups.map((group) => {
				const paths = allPathsIn(group);
				const allOn = paths.length > 0 && paths.every((p) => selected.has(p));
				return (
					<div key={group.label} className="flex flex-col gap-1">
						<div className="flex items-center gap-2 px-1 py-1 text-fg-muted">
							<Checkbox
								checked={allOn}
								onCheckedChange={(v) => onToggleGroup(paths, v === true)}
								aria-label={`Select all in ${group.label}`}
							/>
							<span className="text-sm">{group.label}</span>
							<span className="ml-auto text-xs">
								{paths.length} {paths.length === 1 ? "clip" : "clips"}
							</span>
						</div>
						<ul className="flex flex-col">
							{group.clips.map((clip) => (
								<li
									key={clip.path}
									className="flex items-center gap-2 rounded-[var(--r)] px-1 py-1.5 hover:bg-surface-2"
								>
									<Checkbox
										checked={selected.has(clip.path)}
										onCheckedChange={(v) => onToggle(clip.path, v === true)}
										aria-label={`Import ${clip.name}`}
									/>
									{clip.is_video ? (
										<FilmSlateIcon size={16} className="text-fg-faint" />
									) : (
										<FileIcon size={16} className="text-fg-faint" />
									)}
									<span
										className={
											clip.is_video
												? "truncate text-sm"
												: "truncate text-fg-muted text-sm"
										}
									>
										{clip.name}
									</span>
									<span className="ml-auto font-mono text-fg-muted text-xs tabular-nums">
										{formatDuration(clip.duration_s)}
									</span>
									<span className="w-20 text-right font-mono text-fg-muted text-xs tabular-nums">
										{formatBytes(clip.size)}
									</span>
								</li>
							))}
						</ul>
					</div>
				);
			})}
		</div>
	);
}
