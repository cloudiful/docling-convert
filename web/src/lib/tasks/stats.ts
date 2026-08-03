import type { Task } from '$lib/types/tasks';

export interface ProgressStats {
	totalTasks: number;
	completedTasks: number;
	partialTasks: number;
	failedTasks: number;
	pendingTasks: number;
	processingTasks: number;
	totalChunks: number;
	completedChunks: number;
	estimatedRemainingMs: number | null;
}

export function computeProgressStats(
	tasks: Task[],
	taskStartTimes: Record<string, Date>
): ProgressStats {
	const processingTasks = tasks.filter((task) => task.status === 'processing');
	const totalTasks = tasks.length;
	const completedTasks = tasks.filter((task) => task.status === 'completed').length;
	const partialTasks = tasks.filter((task) => task.status === 'partial').length;
	const failedTasks = tasks.filter((task) => task.status === 'failed').length;
	const pendingTasks = tasks.filter((task) => task.status === 'pending').length;

	let totalChunks = 0;
	let completedChunks = 0;
	for (const task of tasks) {
		totalChunks += task.total_chunks;
		completedChunks += Math.min(task.completed_chunks, task.total_chunks);
	}

	let estimatedRemainingMs: number | null = null;
	if (processingTasks.length > 0) {
		const now = Date.now();
		let totalElapsed = 0;
		let totalProcessed = 0;

		for (const task of processingTasks) {
			const startTime = taskStartTimes[task.id];
			if (!startTime) {
				continue;
			}

			const elapsed = now - startTime.getTime();
			if (task.total_chunks === 0) {
				continue;
			}

			const processed =
				task.completed_chunks > 0 ? task.completed_chunks : (task.progress / 100) * task.total_chunks;
			totalElapsed += elapsed;
			totalProcessed += processed;
		}

		if (totalProcessed > 0) {
			const avgTimePerChunk = totalElapsed / totalProcessed;
			const remainingChunks = totalChunks - completedChunks;
			estimatedRemainingMs = avgTimePerChunk * remainingChunks;
		}
	}

	return {
		totalTasks,
		completedTasks,
		partialTasks,
		failedTasks,
		pendingTasks,
		processingTasks: processingTasks.length,
		totalChunks,
		completedChunks,
		estimatedRemainingMs
	};
}

export function syncTaskStartTimes(
	current: Record<string, Date>,
	tasks: Task[]
): Record<string, Date> {
	const next = { ...current };

	for (const task of tasks) {
		if (task.status === 'processing' && !next[task.id]) {
			next[task.id] = new Date();
		}
	}

	return next;
}

export function formatTimeRemaining(ms: number | null): string {
	if (ms === null || ms <= 0) {
		return '--';
	}

	const seconds = Math.floor(ms / 1000);
	const minutes = Math.floor(seconds / 60);
	const hours = Math.floor(minutes / 60);

	if (hours > 0) {
		return `${hours}h ${minutes % 60}m`;
	}

	if (minutes > 0) {
		return `${minutes}m ${seconds % 60}s`;
	}

	return `${seconds}s`;
}
