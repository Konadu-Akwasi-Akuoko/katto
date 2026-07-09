import type {
	Idea,
	IdeaCreate,
	IdeaPatch,
	PromoteResult,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Idea, IdeaCreate, IdeaPatch, PromoteResult };

export const ideasKeys = {
	all: ["ideas"] as const,
	byStatus: (status: string) => ["ideas", status] as const,
};

/** Ideas with the given status (e.g. `backlog`), newest-first by capture time. */
export const listIdeas = (status: string): Promise<Idea[]> =>
	unwrap(commands.listIdeas(status));

/** Capture a new idea into the backlog (`type='manual'`, `status='backlog'`). */
export const createIdea = (input: IdeaCreate): Promise<Idea> =>
	unwrap(commands.createIdea(input));

/** Patch an idea's editable fields; a `null` field leaves that column unchanged. */
export const updateIdea = (id: string, patch: IdeaPatch): Promise<Idea> =>
	unwrap(commands.updateIdea(id, patch));

/** Discard an idea; the row is retained as an audit trail (`status='discarded'`). */
export const discardIdea = (id: string): Promise<null> =>
	unwrap(commands.discardIdea(id));

/** Promote a backlog idea into a real project folder in one transaction. */
export const promoteIdea = (id: string): Promise<PromoteResult> =>
	unwrap(commands.promoteIdea(id));
