<script lang="ts">
	import { t } from '$lib/i18n';
	import type { TaskConfig } from '$lib/types/tasks';

	let {
		show,
		config,
		onToggle,
		onPatch
	}: {
		show: boolean;
		config: TaskConfig;
		onToggle: (show: boolean) => void;
		onPatch: (patch: Partial<TaskConfig>) => void;
	} = $props();
</script>

{#if show}
	<div
		class="mb-6 bg-white dark:bg-gray-800 rounded-lg shadow p-6 prose prose-sm max-w-none"
	>
		<div class="flex items-center justify-between mb-4 not-prose">
			<h2 class="text-lg font-semibold text-gray-900 dark:text-white m-0">{$t.settings.title}</h2>
			<button
				onclick={() => onToggle(false)}
				class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
				aria-label="Close settings"
			>
				<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M6 18L18 6M6 6l12 12"
					/>
				</svg>
			</button>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
			<div>
				<label
					for="format-select"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					{$t.settings.format}
				</label>
				<select
					id="format-select"
					value={config.format}
					onchange={(event) =>
						onPatch({ format: (event.currentTarget as HTMLSelectElement).value })}
					class="form-select w-full px-3 py-2 border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded-lg"
				>
					<option value="md">{$t.settings.formatMd}</option>
					<option value="json">{$t.settings.formatJson}</option>
					<option value="text">{$t.settings.formatText}</option>
					<option value="html">{$t.settings.formatHtml}</option>
					<option value="doctags">{$t.settings.formatDoctags}</option>
				</select>
			</div>

			<div>
				<label
					for="pages-input"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					{$t.settings.pagesPerFile}
				</label>
				<input
					id="pages-input"
					type="number"
					min="1"
					max="100"
					value={config.pages_per_file}
					oninput={(event) =>
						onPatch({ pages_per_file: Number((event.currentTarget as HTMLInputElement).value) })}
					class="form-input w-full px-3 py-2 border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded-lg"
				/>
			</div>

			<div>
				<label
					for="batch-input"
					class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
				>
					{$t.settings.batchSize}
				</label>
				<input
					id="batch-input"
					type="number"
					min="1"
					max="20"
					value={config.batch_size}
					oninput={(event) =>
						onPatch({ batch_size: Number((event.currentTarget as HTMLInputElement).value) })}
					class="form-input w-full px-3 py-2 border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded-lg"
				/>
			</div>

			<div class="flex items-center space-x-4">
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.split_input}
						onchange={(event) =>
							onPatch({ split_input: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600 border-gray-300 dark:border-gray-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.splitInput}</span>
				</label>
			</div>

			<div class="flex items-center space-x-4">
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.split_by_bookmark}
						onchange={(event) =>
							onPatch({ split_by_bookmark: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600 border-gray-300 dark:border-gray-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">
						{$t.settings.splitByBookmark}
					</span>
				</label>
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.chunking}
						onchange={(event) =>
							onPatch({ chunking: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600 border-gray-300 dark:border-gray-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.chunking}</span>
				</label>
			</div>
		</div>
	</div>
{:else}
	<div class="mb-4">
		<button
			onclick={() => onToggle(true)}
			class="inline-flex items-center px-4 py-2 bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
		>
			<svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
				/>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
				/>
			</svg>
			{$t.header.settings}
		</button>
	</div>
{/if}
