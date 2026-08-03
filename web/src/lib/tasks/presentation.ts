import type { Locale } from '$lib/i18n';
import type { TaskStatus } from '$lib/types/tasks';

export function getStatusColor(status: TaskStatus): string {
	switch (status) {
		case 'pending':
			return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200';
		case 'processing':
			return 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200';
		case 'completed':
			return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200';
		case 'partial':
			return 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200';
		case 'failed':
			return 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200';
		case 'skipped':
			return 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200';
	}
}

export function formatTaskDate(dateStr: string, locale: Locale): string {
	const date = new Date(dateStr);
	return date.toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US', {
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit'
	});
}
