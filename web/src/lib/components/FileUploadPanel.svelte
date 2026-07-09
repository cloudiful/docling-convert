<script lang="ts">
	import { t } from '$lib/i18n';

	let {
		isUploading,
		uploadProgress,
		dragOver,
		onDragChange,
		onFileSelected
	}: {
		isUploading: boolean;
		uploadProgress: number;
		dragOver: boolean;
		onDragChange: (dragOver: boolean) => void;
		onFileSelected: (file: File) => void | Promise<void>;
	} = $props();

	const supportedExtensions = [
		'.pdf',
		'.docx',
		'.pptx',
		'.html',
		'.htm',
		'.xhtml',
		'.adoc',
		'.asciidoc',
		'.asc',
		'.md',
		'.markdown',
		'.csv',
		'.xlsx',
		'.odt',
		'.ods',
		'.odp',
		'.epub',
		'.eml',
		'.msg',
		'.png',
		'.jpg',
		'.jpeg',
		'.gif',
		'.bmp',
		'.tif',
		'.tiff',
		'.webp',
		'.svg',
		'.txt'
	];
	const acceptedFormats = supportedExtensions.join(',');

	function supportsFile(file: File): boolean {
		const name = file.name.toLowerCase();
		return supportedExtensions.some((extension) => name.endsWith(extension));
	}

	function handleChange(event: Event) {
		const file = (event.currentTarget as HTMLInputElement).files?.[0];
		if (!file || !supportsFile(file)) {
			return;
		}

		void onFileSelected(file);
		(event.currentTarget as HTMLInputElement).value = '';
	}

	function handleDrop(event: DragEvent) {
		event.preventDefault();
		onDragChange(false);

		const file = event.dataTransfer?.files?.[0];
		if (file && supportsFile(file)) {
			void onFileSelected(file);
		}
	}
</script>

<div
	class={`bg-white dark:bg-gray-800 rounded-lg shadow p-6 prose prose-sm max-w-none transition-colors ${
		dragOver ? 'ring-2 ring-blue-500 ring-offset-2 dark:ring-offset-gray-900' : ''
	}`}
	ondrop={handleDrop}
	ondragover={(event) => {
		event.preventDefault();
		onDragChange(true);
	}}
	ondragleave={() => onDragChange(false)}
	role="region"
	aria-label="File upload"
>
	<h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4 not-prose">{$t.upload.title}</h2>
	<div class="flex flex-col items-center justify-center min-h-[200px]">
		{#if isUploading}
			<div class="space-y-3 text-center">
				<svg class="w-12 h-12 mx-auto text-blue-500 animate-spin" fill="none" viewBox="0 0 24 24">
					<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
					<path
						class="opacity-75"
						fill="currentColor"
						d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
					/>
				</svg>
				<p class="text-gray-600 dark:text-gray-400">{$t.upload.uploading}</p>
				<div class="w-64 bg-gray-200 dark:bg-gray-700 rounded-full h-2">
					<div class="bg-blue-600 h-2 rounded-full transition-all duration-300" style={`width: ${uploadProgress}%`}></div>
				</div>
				<p class="text-sm text-gray-500 dark:text-gray-500">{uploadProgress}%</p>
			</div>
		{:else}
			<svg class="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
				/>
			</svg>
			<p class="text-gray-600 dark:text-gray-400 mb-2">{$t.upload.dragDrop}</p>
			<p class="text-gray-500 dark:text-gray-500 text-sm mb-4">{$t.upload.formats}</p>
			<label class="cursor-pointer inline-flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors not-prose">
				<svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
					/>
				</svg>
				{$t.upload.browse}
				<input type="file" accept={acceptedFormats} onchange={handleChange} class="hidden" />
			</label>
		{/if}
	</div>
</div>
