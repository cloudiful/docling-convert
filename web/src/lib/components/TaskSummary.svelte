<script lang="ts">
	import { locale, t } from '$lib/i18n';
	import { formatTimeRemaining, type ProgressStats } from '$lib/tasks/stats';

	let { stats }: { stats: ProgressStats } = $props();
</script>

{#if stats.totalTasks > 0}
	<div class="mb-6 bg-white dark:bg-gray-800 rounded-lg shadow p-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<div class="flex flex-wrap items-center gap-6">
				<div class="text-center">
					<div class="text-2xl font-bold text-gray-900 dark:text-white">{stats.totalTasks}</div>
					<div class="text-xs text-gray-500 dark:text-gray-400">
						{#if $locale === 'zh'}总计{:else}Total{/if}
					</div>
				</div>
				<div class="text-center">
					<div class="text-2xl font-bold text-green-600 dark:text-green-400">{stats.completedTasks}</div>
					<div class="text-xs text-gray-500 dark:text-gray-400">{$t.tasks.status.completed}</div>
				</div>
				<div class="text-center">
					<div class="text-2xl font-bold text-amber-600 dark:text-amber-400">{stats.partialTasks}</div>
					<div class="text-xs text-gray-500 dark:text-gray-400">{$t.tasks.status.partial}</div>
				</div>
				<div class="text-center">
					<div class="text-2xl font-bold text-blue-600 dark:text-blue-400">{stats.processingTasks}</div>
					<div class="text-xs text-gray-500 dark:text-gray-400">{$t.tasks.status.processing}</div>
				</div>
				<div class="text-center">
					<div class="text-2xl font-bold text-red-600 dark:text-red-400">{stats.failedTasks}</div>
					<div class="text-xs text-gray-500 dark:text-gray-400">{$t.tasks.status.failed}</div>
				</div>
			</div>

			<div class="flex items-center gap-4">
				<div class="w-48">
					<div class="flex justify-between text-sm mb-1">
						<span class="text-gray-600 dark:text-gray-400">
							{stats.completedChunks} / {stats.totalChunks} {$t.tasks.chunks}
						</span>
						<span class="text-gray-600 dark:text-gray-400">
							{Math.round((stats.completedChunks / Math.max(stats.totalChunks, 1)) * 100)}%
						</span>
					</div>
					<div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
						<div
							class="bg-blue-600 dark:bg-blue-500 h-2 rounded-full transition-all duration-500"
							style={`width: ${
								stats.totalChunks > 0 ? (stats.completedChunks / stats.totalChunks) * 100 : 0
							}%`}
						></div>
					</div>
				</div>

				{#if stats.processingTasks > 0 && stats.estimatedRemainingMs}
					<div class="text-right">
						<div class="text-sm text-gray-500 dark:text-gray-400">{$t.progress.estimatedTime}</div>
						<div class="text-lg font-semibold text-gray-900 dark:text-white">
							{formatTimeRemaining(stats.estimatedRemainingMs)}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>
{/if}
