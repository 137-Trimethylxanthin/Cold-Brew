// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	type SpotifyWebPlaybackError = {
		message: string;
	};

	type SpotifyWebPlaybackState = {
		paused: boolean;
		position: number;
		duration: number;
		track_window: {
			current_track: {
				name: string;
				uri: string;
				album: { name: string };
				artists: Array<{ name: string }>;
			};
		};
	};

	type SpotifyPlayer = {
		connect(): Promise<boolean>;
		disconnect(): void;
		pause(): Promise<void>;
		resume(): Promise<void>;
		nextTrack(): Promise<void>;
		previousTrack(): Promise<void>;
		seek(positionMs: number): Promise<void>;
		setVolume(volume: number): Promise<void>;
		getCurrentState(): Promise<SpotifyWebPlaybackState | null>;
		activateElement?: () => Promise<void>;
		addListener(event: 'ready', callback: ({ device_id }: { device_id: string }) => void): boolean;
		addListener(
			event: 'not_ready',
			callback: ({ device_id }: { device_id: string }) => void
		): boolean;
		addListener(
			event: 'player_state_changed',
			callback: (state: SpotifyWebPlaybackState | null) => void
		): boolean;
		addListener(
			event: 'initialization_error' | 'authentication_error' | 'account_error' | 'playback_error',
			callback: (error: SpotifyWebPlaybackError) => void
		): boolean;
	};

	type SpotifyPlayerConstructor = new (options: {
		name: string;
		getOAuthToken: (callback: (token: string) => void) => void;
		volume?: number;
	}) => SpotifyPlayer;

	interface Window {
		onSpotifyWebPlaybackSDKReady?: () => void;
		Spotify?: {
			Player: SpotifyPlayerConstructor;
		};
	}
}

export {};
