export type TaskStatus = 'pending' | 'processing' | 'completed' | 'partial' | 'failed' | 'skipped';
export type ChunkerKind = 'none' | 'hybrid' | 'hierarchical';
export type PipelineKind = 'legacy' | 'standard' | 'vlm' | 'asr';

export interface ChunkingOptions {
	use_markdown_tables: boolean;
	use_markdown_images: boolean;
	image_placeholder: string;
	include_raw_text: boolean;
	max_tokens: number | null;
	tokenizer: string | null;
	merge_peers: boolean;
}

export interface TaskConfig {
	format: string;
	input_format?: string | null;
	chunker: ChunkerKind;
	chunking_options: ChunkingOptions;
	pipeline?: PipelineKind | null;
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
	chunker: 'none',
	chunking_options: {
		use_markdown_tables: false,
		use_markdown_images: false,
		image_placeholder: '![IMAGE]',
		include_raw_text: false,
		max_tokens: null,
		tokenizer: 'sentence-transformers/all-MiniLM-L6-v2',
		merge_peers: true
	},
	pipeline: null
};
