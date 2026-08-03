import type { Task, TaskConfig } from '$lib/types/tasks';

const API_BASE = '/api';

export async function fetchTasks(): Promise<Task[]> {
	const response = await fetch(`${API_BASE}/tasks`);
	if (!response.ok) {
		throw new Error('Failed to fetch tasks');
	}

	const data = await response.json();
	return data.tasks || data || [];
}

export async function uploadTaskFile(
	file: File,
	config: TaskConfig,
	onProgress: (progress: number) => void
): Promise<void> {
	const formData = new FormData();
	formData.append('file', file);
	formData.append('format', config.format);
	if (config.input_format) {
		formData.append('input_format', config.input_format);
	}
	formData.append('chunker', config.chunker);
	if (config.pipeline) {
		formData.append('pipeline', config.pipeline);
	}
	formData.append('use_markdown_tables', config.chunking_options.use_markdown_tables.toString());
	formData.append('use_markdown_images', config.chunking_options.use_markdown_images.toString());
	formData.append('image_placeholder', config.chunking_options.image_placeholder);
	formData.append('include_raw_text', config.chunking_options.include_raw_text.toString());
	formData.append('merge_peers', config.chunking_options.merge_peers.toString());
	if (config.chunking_options.max_tokens !== null) {
		formData.append('max_tokens', config.chunking_options.max_tokens.toString());
	}
	if (config.chunking_options.tokenizer) {
		formData.append('tokenizer', config.chunking_options.tokenizer);
	}

	await new Promise<void>((resolve, reject) => {
		const xhr = new XMLHttpRequest();

		xhr.upload.addEventListener('progress', (event) => {
			if (event.lengthComputable) {
				onProgress(Math.round((event.loaded / event.total) * 100));
			}
		});

		xhr.addEventListener('load', () => {
			if (xhr.status >= 200 && xhr.status < 300) {
				resolve();
				return;
			}

			reject(new Error(buildUploadError(xhr)));
		});

		xhr.addEventListener('error', () => {
			reject(new Error('Upload failed: Network error'));
		});

		xhr.open('POST', `${API_BASE}/upload`);
		xhr.send(formData);
	});
}

export async function submitTaskUrl(url: string, config: TaskConfig): Promise<void> {
	const response = await fetch(`${API_BASE}/convert/url`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ url, config })
	});

	if (response.ok) {
		return;
	}

	const data = await response.json().catch(() => ({}));
	throw new Error(data.error || 'Failed to submit URL');
}

export async function deleteTask(taskId: string): Promise<void> {
	const response = await fetch(`${API_BASE}/tasks/${taskId}`, { method: 'DELETE' });
	if (!response.ok) {
		throw new Error('Failed to delete task');
	}
}

function buildUploadError(xhr: XMLHttpRequest): string {
	let message = `Upload failed: ${xhr.statusText}`;

	try {
		const response = JSON.parse(xhr.responseText);
		message += ` - ${response.error || response.message || ''}`;
	} catch {
		return message;
	}

	return message;
}
