import { writable } from 'svelte/store';
import type { AbRepeatState, CrossfeedState, EqState, PlaybackSettings, PlaybackStatus, QueueSnapshot, SleepTimerState, Song } from '$lib/types';
import { emptySong } from '$lib/playback';

export const playbackStatus = writable<PlaybackStatus | null>(null);
export const queueSnapshot = writable<QueueSnapshot | null>(null);
export const currentSong = writable<Song>(emptySong());
export const playerError = writable<string>('');
export const volume = writable<number>(1);
export const playbackSettings = writable<PlaybackSettings>({
	crossfade_duration_ms: null,
	playback_speed: 1.0,
	mono_downmix: false,
	preamp_gain_db: 0.0,
	replay_gain_mode: 'off'
});
export const eqState = writable<EqState>({ bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], preset_name: 'Flat' });
export const crossfeedState = writable<CrossfeedState>({ level: 'off' });
export const sleepTimerState = writable<SleepTimerState>({ active: false, remaining_seconds: null, total_seconds: null });
export const abRepeatState = writable<AbRepeatState>({ active: false, loop_start_secs: null, loop_end_secs: null });
