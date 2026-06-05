import type { PlaybackStatus, Song } from '$lib/types';

export function formatDuration(ms: number): string {
	if (!ms) return '0:00';
	const totalSeconds = Math.floor(ms / 1000);
	const minutes = Math.floor(totalSeconds / 60);
	const seconds = totalSeconds % 60;
	return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

export function formatSampleRate(rate: number): string {
	const value = rate / 1000;
	return `${Number.isInteger(value) ? value : value.toFixed(1)} kHz`;
}

export function formatSource(source: string): string {
	const mapping: Record<string, string> = {
		lastfm: 'Last.fm',
		qobuz: 'Qobuz',
		tidal: 'TIDAL',
		youtube: 'YouTube',
		spotify: 'Spotify',
		jellyfin: 'Jellyfin',
		local: 'Local'
	};
	return mapping[source] ?? source;
}

export function playbackQualityLabel(status: PlaybackStatus): string {
	const parts: string[] = [];
	if (status.source_format) parts.push(status.source_format.toUpperCase());
	if (status.source_is_lossless !== null) parts.push(status.source_is_lossless ? 'lossless' : 'lossy');
	if (status.source_sample_rate) parts.push(formatSampleRate(status.source_sample_rate));
	if (status.source_channels) parts.push(`${status.source_channels} ch`);
	if (status.output_sample_rate) parts.push(`out ${formatSampleRate(status.output_sample_rate)}`);
	if (status.replay_gain_db !== null) {
		parts.push(
			`RG ${status.replay_gain_source ?? status.replay_gain_mode} ${formatDb(status.replay_gain_db)}`
		);
	}
	if (status.quality_warnings[0]) parts.push(status.quality_warnings[0]);
	return parts.join(' / ') || 'Local file';
}

export function formatDb(value: number): string {
	return `${value > 0 ? '+' : ''}${value.toFixed(1)} dB`;
}

export function toErrorMessage(error: unknown): string {
	if (typeof error === 'string') return error;
	if (error instanceof Error) return error.message;
	return 'Unexpected playback error.';
}

export function emptySong(): Song {
	return {
		title: 'Nothing playing',
		artist: '',
		album: '',
		duration: 0,
		id: ''
	};
}

export function titleFromPath(path: string): string {
	const fileName = path.split(/[\\/]/).pop() ?? 'Untitled';
	return fileName.replace(/\.[^.]+$/, '') || 'Untitled';
}
