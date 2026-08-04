import { ArrowLeftIcon, FolderOpenIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useId } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DateInput } from "@/components/ui/date-input";
import { Label } from "@/components/ui/label";
import { StatusChip } from "@/components/ui/status-chip";
import { FootageCard } from "@/features/projects/detail/footage-card";
import { ThumbnailsCard } from "@/features/projects/detail/thumbnails-card";
import { VfxCard } from "@/features/projects/detail/vfx-card";
import { relativeMtime } from "@/features/projects/model/format";
import type { FolderFreshness, Project } from "@/lib/ipc/projects";
import {
	getProject,
	projectsKeys,
	revealProjectFolder,
	setProjectDates,
} from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";

export function ProjectDetail({ slug }: { slug: string }) {
	const queryClient = useQueryClient();
	const back = useUiStore((s) => s.setSelectedProjectSlug);
	const { data } = useQuery({
		queryKey: projectsKeys.detail(slug),
		queryFn: () => getProject(slug),
		enabled: !!slug,
	});

	const dates = useMutation({
		mutationFn: (vars: { shoot: string | null; publish: string | null }) =>
			setProjectDates(slug, vars.shoot, vars.publish),
		onSuccess: () => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		},
	});
	const reveal = useMutation({
		mutationFn: (subfolder: string | null) =>
			revealProjectFolder(slug, subfolder),
	});

	return (
		<div className="flex flex-col gap-4 p-6">
			<Button
				variant="ghost"
				size="sm"
				className="self-start"
				onClick={() => back(null)}
			>
				<ArrowLeftIcon />
				Projects
			</Button>

			{data === undefined ? null : (
				<>
					<header className="flex flex-wrap items-center gap-3">
						<h1 className="mr-auto font-serif text-2xl">
							{data.project.title}
						</h1>
						<StatusChip status={data.project.status} />
						<Button
							variant="secondary"
							size="sm"
							onClick={() => reveal.mutate(null)}
						>
							<FolderOpenIcon />
							Reveal in Finder
						</Button>
					</header>

					<ManifestCard
						project={data.project}
						manifestError={data.manifest_error}
					/>

					<DatesCard
						project={data.project}
						pending={dates.isPending}
						onSetDates={(vars) => dates.mutate(vars)}
					/>

					<FreshnessCard
						freshness={data.freshness}
						onReveal={(subfolder) => reveal.mutate(subfolder)}
					/>

					<FootageCard slug={slug} />

					<VfxCard slug={slug} />

					<ThumbnailsCard slug={slug} />
				</>
			)}
		</div>
	);
}

function ManifestCard({
	project,
	manifestError,
}: {
	project: Project;
	manifestError: string | null;
}) {
	return (
		<Card>
			<CardHeader>
				<CardTitle>Manifest</CardTitle>
			</CardHeader>
			<CardContent>
				{manifestError !== null ? (
					<div className="flex flex-col gap-2">
						<Badge variant="failed" className="self-start">
							<span className="dot" />
							Invalid manifest
						</Badge>
						<pre className="overflow-x-auto rounded-md border bg-surface-2 px-3 py-2 font-mono text-xs text-fg-muted">
							{manifestError}
						</pre>
					</div>
				) : (
					<dl className="grid gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
						<Field label="Slug" value={project.slug} mono />
						<Field label="Target NLE" value={project.target_nle} />
						<Field label="Root" value={project.root_path} mono />
					</dl>
				)}
			</CardContent>
		</Card>
	);
}

function Field({
	label,
	value,
	mono = false,
}: {
	label: string;
	value: string;
	mono?: boolean;
}) {
	return (
		<div className="flex min-w-0 flex-col gap-0.5">
			<dt className="text-xs text-fg-faint">{label}</dt>
			<dd
				className={
					mono ? "truncate font-mono text-fg tabular-nums" : "truncate text-fg"
				}
			>
				{value}
			</dd>
		</div>
	);
}

function DatesCard({
	project,
	pending,
	onSetDates,
}: {
	project: Project;
	pending: boolean;
	onSetDates: (vars: { shoot: string | null; publish: string | null }) => void;
}) {
	return (
		<Card>
			<CardHeader>
				<CardTitle>Dates</CardTitle>
			</CardHeader>
			<CardContent className="flex flex-wrap gap-6">
				<DateField
					label="Shoot"
					value={project.shoot_date}
					disabled={pending}
					onChange={(shoot) =>
						onSetDates({ shoot, publish: project.publish_date })
					}
				/>
				<DateField
					label="Publish"
					value={project.publish_date}
					disabled={pending}
					onChange={(publish) =>
						onSetDates({ shoot: project.shoot_date, publish })
					}
				/>
			</CardContent>
		</Card>
	);
}

function DateField({
	label,
	value,
	disabled,
	onChange,
}: {
	label: string;
	value: string | null;
	disabled: boolean;
	onChange: (value: string | null) => void;
}) {
	const id = useId();
	return (
		<div className="flex flex-col gap-1">
			<Label htmlFor={id}>{label}</Label>
			<DateInput
				id={id}
				label={label}
				value={value}
				onValueChange={onChange}
				disabled={disabled}
			/>
		</div>
	);
}

function FreshnessCard({
	freshness,
	onReveal,
}: {
	freshness: FolderFreshness[];
	onReveal: (subfolder: string) => void;
}) {
	const now = new Date();
	return (
		<Card>
			<CardHeader>
				<CardTitle>Folders</CardTitle>
			</CardHeader>
			<CardContent>
				<ul className="grid gap-2 sm:grid-cols-2">
					{freshness.map((folder) => (
						<li
							key={folder.subfolder}
							className="flex items-center gap-3 rounded-md border bg-surface px-3 py-2"
						>
							<div className="flex min-w-0 flex-1 flex-col gap-0.5">
								<span className="truncate font-mono text-sm text-fg">
									{folder.subfolder}
								</span>
								<span className="flex gap-3 font-mono text-xs text-fg-muted tabular-nums">
									<span>
										{folder.file_count}{" "}
										{folder.file_count === 1 ? "file" : "files"}
									</span>
									<span>{relativeMtime(folder.latest_mtime, now)}</span>
								</span>
							</div>
							<Button
								variant="ghost"
								size="icon-sm"
								aria-label={`Reveal ${folder.subfolder} in Finder`}
								onClick={() => onReveal(folder.subfolder)}
							>
								<FolderOpenIcon />
							</Button>
						</li>
					))}
				</ul>
			</CardContent>
		</Card>
	);
}
