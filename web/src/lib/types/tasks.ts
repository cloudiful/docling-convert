export type TaskStatus = 'pending' | 'processing' | 'completed' | 'failed';

export interface TaskConfig {
	format: string;
	input_format?: string | null;
	pages_per_file: number;
	split_input: boolean;
	split_by_bookmark: boolean;
	chunking: boolean;
	batch_size: number;
}

export interface Task {
	id: string;
	filename: string;
	status: TaskStatus;
	progress: number;
	message?: string;
	output_url?: string;
	total_chunks: number;
	completed_chunks: number;
	created_at: string;
	completed_at?: string;
	config: TaskConfig;
	started_at?: string;
}

export const defaultTaskConfig: TaskConfig = {
	format: 'md',
	input_format: null,
	pages_per_file: 5,
	split_input: true,
	split_by_bookmark: false,
	chunking: false,
	batch_size: 2
};
