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

	const inputFormats = [
		['pdf', 'PDF'],
		['doc', 'DOC'],
		['docx', 'DOCX'],
		['ppt', 'PPT'],
		['pptx', 'PPTX'],
		['html', 'HTML'],
		['asciidoc', 'AsciiDoc'],
		['md', 'Markdown'],
		['csv', 'CSV'],
		['xlsx', 'XLSX'],
		['xls', 'XLS'],
		['odt', 'ODT'],
		['ods', 'ODS'],
		['odp', 'ODP'],
		['epub', 'EPUB'],
		['email', 'Email'],
		['image', 'Image'],
		['xml_uspto', 'XML USPTO'],
		['xml_jats', 'XML JATS'],
		['xml_xbrl', 'XML XBRL'],
		['xml_doclang', 'XML Docling'],
		['mets_gbs', 'METS GBS'],
		['json_docling', 'JSON Docling'],
		['dclx', 'DCLX'],
		['audio', 'Audio'],
		['video', 'Video'],
		['vtt', 'VTT'],
		['boxnote', 'Box Note'],
		['latex', 'LaTeX'],
		['text', 'Text']
	];

	const updateChunking = (patch: Partial<TaskConfig['chunking_options']>) =>
		onPatch({ chunking_options: { ...config.chunking_options, ...patch } });
</script>

{#if show}
	<div class="mb-6 rounded-lg bg-white p-6 shadow dark:bg-gray-800">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-lg font-semibold text-gray-900 dark:text-white">{$t.settings.title}</h2>
			<button
				onclick={() => onToggle(false)}
				class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
				aria-label={$t.settings.close}
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</button>
		</div>

		<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
			<div>
				<label for="format-select" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
					{$t.settings.format}
				</label>
				<select
					id="format-select"
					value={config.format}
					onchange={(event) => onPatch({ format: (event.currentTarget as HTMLSelectElement).value })}
					class="form-select w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				>
					<option value="md">{$t.settings.formatMd}</option>
					<option value="json">{$t.settings.formatJson}</option>
					<option value="yaml">{$t.settings.formatYaml}</option>
					<option value="html">{$t.settings.formatHtml}</option>
					<option value="html_split_page">{$t.settings.formatHtmlSplitPage}</option>
					<option value="text">{$t.settings.formatText}</option>
					<option value="doctags">{$t.settings.formatDoctags}</option>
					<option value="vtt">{$t.settings.formatVtt}</option>
					<option value="doclang">{$t.settings.formatDoclang}</option>
					<option value="dclx">{$t.settings.formatDclx}</option>
					<option value="chunks">{$t.settings.formatChunks}</option>
				</select>
			</div>

			<div>
				<label for="input-format-select" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
					{$t.settings.inputFormat}
				</label>
				<select
					id="input-format-select"
					value={config.input_format ?? ''}
					onchange={(event) => {
						const value = (event.currentTarget as HTMLSelectElement).value;
						onPatch({ input_format: value || null });
					}}
					class="form-select w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				>
					<option value="">{$t.settings.inputFormatAuto}</option>
					{#each inputFormats as [value, label]}
						<option {value}>{label}</option>
					{/each}
				</select>
			</div>

			<div>
				<label for="chunker-select" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
					{$t.settings.chunker}
				</label>
				<select
					id="chunker-select"
					value={config.chunker}
					onchange={(event) =>
						onPatch({ chunker: (event.currentTarget as HTMLSelectElement).value as TaskConfig['chunker'] })}
					class="form-select w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				>
					<option value="none">{$t.settings.chunkerNone}</option>
					<option value="hybrid">{$t.settings.chunkerHybrid}</option>
					<option value="hierarchical">{$t.settings.chunkerHierarchical}</option>
				</select>
			</div>

			<div>
				<label for="pipeline-select" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
					{$t.settings.pipeline}
				</label>
				<select
					id="pipeline-select"
					value={config.pipeline ?? ''}
					onchange={(event) => {
						const value = (event.currentTarget as HTMLSelectElement).value;
						onPatch({ pipeline: (value || null) as TaskConfig['pipeline'] });
					}}
					class="form-select w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
				>
					<option value="">{$t.settings.pipelineDefault}</option>
					<option value="legacy">{$t.settings.pipelineLegacy}</option>
					<option value="standard">{$t.settings.pipelineStandard}</option>
					<option value="vlm">{$t.settings.pipelineVlm}</option>
					<option value="asr">{$t.settings.pipelineAsr}</option>
				</select>
			</div>
		</div>

		{#if config.chunker !== 'none'}
			<div class="mt-5 grid grid-cols-1 gap-4 border-t border-gray-200 pt-5 dark:border-gray-700 md:grid-cols-2">
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.chunking_options.use_markdown_tables}
						onchange={(event) => updateChunking({ use_markdown_tables: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.useMarkdownTables}</span>
				</label>
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.chunking_options.use_markdown_images}
						onchange={(event) => updateChunking({ use_markdown_images: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.useMarkdownImages}</span>
				</label>
				<div>
					<label for="image-placeholder" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
						{$t.settings.imagePlaceholder}
					</label>
					<input
						id="image-placeholder"
						type="text"
						value={config.chunking_options.image_placeholder}
						oninput={(event) => updateChunking({ image_placeholder: (event.currentTarget as HTMLInputElement).value })}
						class="form-input w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
					/>
				</div>
				<label class="flex items-center">
					<input
						type="checkbox"
						checked={config.chunking_options.include_raw_text}
						onchange={(event) => updateChunking({ include_raw_text: (event.currentTarget as HTMLInputElement).checked })}
						class="form-checkbox rounded text-blue-600"
					/>
					<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.includeRawText}</span>
				</label>

				{#if config.chunker === 'hybrid'}
					<div>
						<label for="max-tokens" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
							{$t.settings.maxTokens}
						</label>
						<input
							id="max-tokens"
							type="number"
							min="1"
							value={config.chunking_options.max_tokens ?? ''}
							oninput={(event) => {
								const value = (event.currentTarget as HTMLInputElement).value;
								updateChunking({ max_tokens: value ? Number(value) : null });
							}}
							class="form-input w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
						/>
					</div>
					<div>
						<label for="tokenizer" class="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
							{$t.settings.tokenizer}
						</label>
						<input
							id="tokenizer"
							type="text"
							value={config.chunking_options.tokenizer ?? ''}
							oninput={(event) => updateChunking({ tokenizer: (event.currentTarget as HTMLInputElement).value || null })}
							class="form-input w-full rounded-lg border-gray-300 px-3 py-2 dark:border-gray-700 dark:bg-gray-700 dark:text-white"
						/>
					</div>
					<label class="flex items-center">
						<input
							type="checkbox"
							checked={config.chunking_options.merge_peers}
							onchange={(event) => updateChunking({ merge_peers: (event.currentTarget as HTMLInputElement).checked })}
							class="form-checkbox rounded text-blue-600"
						/>
						<span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{$t.settings.mergePeers}</span>
					</label>
				{/if}
			</div>
		{/if}
	</div>
{:else}
	<div class="mb-4">
		<button
			onclick={() => onToggle(true)}
			class="inline-flex items-center rounded-lg bg-gray-100 px-4 py-2 text-gray-700 transition-colors hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
		>
			<svg class="mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317a1.724 1.724 0 013.35 0 1.724 1.724 0 002.573 1.066 1.724 1.724 0 012.37 2.37 1.724 1.724 0 001.065 2.572 1.724 1.724 0 010 3.35 1.724 1.724 0 00-1.066 2.573 1.724 1.724 0 01-2.37 2.37 1.724 1.724 0 00-2.572 1.065 1.724 1.724 0 01-3.35 0 1.724 1.724 0 00-2.573-1.066 1.724 1.724 0 01-2.37-2.37 1.724 1.724 0 00-1.065-2.572 1.724 1.724 0 010-3.35 1.724 1.724 0 001.066-2.573 1.724 1.724 0 012.37-2.37c.996.608 2.296.07 2.572-1.065z" />
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
			</svg>
			{$t.header.settings}
		</button>
	</div>
{/if}
