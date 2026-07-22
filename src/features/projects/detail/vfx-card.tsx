import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { listVfxEffects, vfxKeys } from "@/lib/ipc/vfx";
import { NewEffectDialog } from "./new-effect-dialog";

/**
 * The project detail's Effects card: one tile per `assets/vfx/<effect>/`
 * folder, previewing the latest render via the asset protocol.
 */
export function VfxCard({ slug }: { slug: string }) {
	const [dialogOpen, setDialogOpen] = useState(false);
	const effects = useQuery({
		queryKey: vfxKeys.byProject(slug),
		queryFn: () => listVfxEffects(slug),
	});

	return (
		<Card>
			<CardHeader className="flex flex-row items-center justify-between">
				<CardTitle>Effects</CardTitle>
				<Button
					variant="secondary"
					size="sm"
					onClick={() => setDialogOpen(true)}
				>
					New effect
				</Button>
			</CardHeader>
			<CardContent>
				{effects.data === undefined || effects.data.length === 0 ? (
					<p className="text-sm text-fg-muted">
						No effects yet. New effect opens a Claude session in its folder.
					</p>
				) : (
					<div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
						{effects.data.map((effect) => {
							const latest = effect.renders[0];
							return (
								<div key={effect.effect} className="flex flex-col gap-1.5">
									{latest !== undefined ? (
										<video
											data-testid={`vfx-preview-${effect.effect}`}
											muted
											playsInline
											preload="metadata"
											className="aspect-video w-full rounded-md border bg-term-bg object-cover"
											src={convertFileSrc(`${effect.path}/${latest}`)}
										/>
									) : (
										<div className="flex aspect-video w-full items-center justify-center rounded-md border">
											<span className="text-xs text-fg-faint">
												no render yet
											</span>
										</div>
									)}
									<div className="flex items-baseline justify-between gap-2">
										<span className="truncate text-sm">{effect.effect}</span>
										{effect.renders.length > 1 && (
											<span className="text-xs text-fg-muted tabular-nums">
												{effect.renders.length} renders
											</span>
										)}
									</div>
								</div>
							);
						})}
					</div>
				)}
			</CardContent>
			<NewEffectDialog
				slug={slug}
				open={dialogOpen}
				onOpenChange={setDialogOpen}
			/>
		</Card>
	);
}
