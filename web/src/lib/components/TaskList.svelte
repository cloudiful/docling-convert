<script lang="ts">
	import { locale, t } from '$lib/i18n';
	import { formatTaskDate, getStatusColor } from '$lib/tasks/presentation';
	import type { Task } from '$lib/types/tasks';

	let {
		tasks,
		onDeleteTask
	}: {
		tasks: Task[];
		onDeleteTask: (taskId: string) => void | Promise<void>;
	} = $props();
</script>

<div class="bg-white dark:bg-gray-800 rounded-lg shadow prose prose-sm max-w-none">
	<div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
		<h2 class="text-lg font-semibold text-gray-900 dark:text-white m-0">{$t.tasks.title}</h2>
	</div>

	{#if tasks.length === 0}
		<div class="p-8 text-center text-gray-500 dark:text-gray-400">
			<svg class="w-16 h-16 mx-auto text-gray-300 dark:text-gray-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
				/>
			</svg>
			<p>{$t.tasks.empty}</p>
			<p class="text-sm text-gray-500 dark:text-gray-500 mt-1">{$t.tasks.emptyHint}</p>
		</div>
	{:else}
		<div class="divide-y divide-gray-200 dark:divide-gray-700">
			{#each tasks as task (task.id)}
				<div class="p-6 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
					<div class="flex items-start justify-between">
						<div class="flex-1 min-w-0">
							<div class="flex items-center space-x-3 mb-2">
								<h3 class="text-sm font-medium text-gray-900 dark:text-white truncate">{task.filename}</h3>
								<span class={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${getStatusColor(task.status)}`}>
									{$t.tasks.status[task.status]}
								</span>
							</div>

							{#if task.message}
								<p class="text-sm text-gray-600 dark:text-gray-400 mb-2">{task.message}</p>
							{/if}

							{#if task.status === 'processing'}
								<div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 mb-2">
									<div class="bg-blue-600 dark:bg-blue-500 h-2 rounded-full transition-all duration-300" style={`width: ${task.progress}%`}></div>
								</div>
								<p class="text-xs text-gray-500 dark:text-gray-400">
									{#if task.total_chunks > 0}
										{$t.progress.taskOf
											.replace('{current}', String(task.completed_chunks))
											.replace('{total}', String(task.total_chunks))}
									{:else}
										{$t.tasks.status.processing}
									{/if}
									{#if task.total_chunks > 1}
										<span class="ml-2">({task.completed_chunks || 0}/{$t.tasks.chunks})</span>
									{/if}
								</p>
							{/if}

							<div class="flex items-center space-x-4 text-xs text-gray-500 dark:text-gray-400">
								<span>{$t.tasks.created}: {formatTaskDate(task.created_at, $locale)}</span>
								{#if task.completed_at}
									<span>{$t.tasks.completed}: {formatTaskDate(task.completed_at, $locale)}</span>
								{/if}
							</div>
						</div>

						<div class="flex items-center space-x-2 ml-4">
							{#if (task.status === 'completed' || task.status === 'partial') && task.output_url}
								<a
									href={task.output_url}
									download
									class="inline-flex items-center px-3 py-1.5 bg-green-600 text-white text-sm rounded-lg hover:bg-green-700 transition-colors"
								>
									<svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path
											stroke-linecap="round"
											stroke-linejoin="round"
											stroke-width="2"
											d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
										/>
									</svg>
									{$t.tasks.download}
								</a>
							{/if}
							<button
								onclick={() => void onDeleteTask(task.id)}
								aria-label={`Delete ${task.filename}`}
								class="inline-flex items-center px-3 py-1.5 bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 text-sm rounded-lg hover:bg-red-200 dark:hover:bg-red-900/50 transition-colors"
							>
								<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
									/>
								</svg>
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
