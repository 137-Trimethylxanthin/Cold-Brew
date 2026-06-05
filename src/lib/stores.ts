import { writable } from 'svelte/store';
import type { PlaybackStatus, QueueSnapshot, Song } from '$lib/types';
import { emptySong } from '$lib/playback';

export const playbackStatus = writable<PlaybackStatus | null>(null);
export const queueSnapshot = writable<QueueSnapshot | null>(null);
export const currentSong = writable<Song>(emptySong());
export const playerError = writable<string>('');
export const volume = writable<number>(1);
