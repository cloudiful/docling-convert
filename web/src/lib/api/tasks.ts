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
	formData.append('pages_per_file', config.pages_per_file.toString());
	formData.append('split_input', config.split_input.toString());
	formData.append('split_by_bookmark', config.split_by_bookmark.toString());
	formData.append('chunking', config.chunking.toString());
	formData.append('batch_size', config.batch_size.toString());

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
