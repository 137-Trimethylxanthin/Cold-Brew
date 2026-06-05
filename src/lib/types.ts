export type Song = {
	id: string;
	title: string;
	artist: string;
	album: string;
	duration: number;
	source?: string | null;
	uri?: string | null;
	external_url?: string | null;
	quality?: string | null;
	playable?: boolean | null;
};

export type QueueSnapshot = {
	current_song: Song | null;
	old: Song[];
	upcoming: Song[];
};

export type QueuePlaybackResult = {
	queue: QueueSnapshot;
	playback_status: PlaybackStatus | null;
	message: string | null;
};

export type PlaylistSummary = {
	id: number;
	name: string;
	track_count: number;
};

export type PlaylistDetail = {
	id: number;
	name: string;
	tracks: Song[];
};

export type LyricsResult = {
	source: string;
	synced: boolean;
	content: string;
};

export type MetadataSuggestion = {
	source: string;
	recording_mbid: string;
	title: string;
	artist: string;
	album: string | null;
	first_release_date: string | null;
	length_ms: number | null;
	score: number | null;
	disambiguation: string | null;
};

export type RemoteTrack = {
	source: string;
	id: string;
	uri: string;
	title: string;
	artist: string;
	album: string | null;
	duration_ms: number | null;
	external_url: string | null;
	quality: string | null;
	playable: boolean;
};

export type RemotePlaylist = {
	source: string;
	id: string;
	name: string;
	track_count: number;
	external_url: string | null;
};

export type ListeningHistoryEntry = {
	id: number;
	path: string;
	title: string | null;
	source: string;
	event: string;
	classification: string | null;
	position_ms: number;
	duration_ms: number | null;
	listened_ms: number;
	created_at: string;
};

export type ListeningHistorySummary = {
	path: string;
	title: string | null;
	source: string;
	play_count: number;
	completion_count: number;
	skip_count: number;
	partial_count: number;
	total_listened_ms: number;
	duration_ms: number | null;
	last_played_at: string;
};

export type LibraryTrack = {
	path: string;
	title: string;
	artist: string | null;
	album: string | null;
	genre: string | null;
	track_number: number | null;
	duration_ms: number | null;
	sample_rate: number | null;
	bit_depth: number | null;
	bitrate: number | null;
	file_size: number;
	modified_secs: number | null;
	extension: string;
	has_artwork: boolean;
};

export type ScanSummary = {
	root: string;
	scanned_files: number;
	indexed_tracks: number;
	skipped_files: number;
	tracks: LibraryTrack[];
};

export type JellyfinAccount = {
	base_url: string;
	user_name: string;
	has_password: boolean;
	source: string;
};

export type ProviderCapability = {
	id: string;
	name: string;
	integration_state: string;
	auth_model: string;
	documentation_url: string;
	can_search: boolean;
	can_list_playlists: boolean;
	can_stream_full_tracks: boolean;
	can_stream_previews: boolean;
	can_link_out: boolean;
	can_scrobble: boolean;
	requires_oauth: boolean;
	requires_partner_access: boolean;
	notes: string[];
};

export type ProviderAccount = {
	provider_id: string;
	display_name: string | null;
	has_client_id: boolean;
	has_client_secret: boolean;
	has_api_key: boolean;
	has_api_secret: boolean;
	has_access_token: boolean;
	has_refresh_token: boolean;
	source: string;
};

export type ProviderLoginState = {
	provider_id: string;
	status: string;
	message: string;
	last_error: string | null;
};

export type ProviderLoginStart = {
	provider_id: string;
	authorization_url: string;
	state: string | null;
	message: string;
};

export type LastFmScrobbleStatus = {
	pending_count: number;
	submitted_count: number;
	failed_count: number;
	last_error: string | null;
};

export type PlaybackStatus = {
	state: 'idle' | 'playing' | 'paused' | 'stopped' | 'ended' | string;
	playing: boolean;
	paused: boolean;
	current_path: string | null;
	current_title: string | null;
	position_ms: number;
	duration_ms: number | null;
	volume: number;
	source_format: string | null;
	source_is_lossless: boolean | null;
	source_sample_rate: number | null;
	source_channels: number | null;
	output_sample_rate: number | null;
	output_channels: number | null;
	output_sample_format: string | null;
	output_device_id: string | null;
	output_device_name: string | null;
	quality_warnings: string[];
	replay_gain_mode: 'off' | 'track' | 'album' | string;
	replay_gain_db: number | null;
	replay_gain_source: string | null;
};

export type AudioOutputDevice = {
	id: string;
	name: string;
	selected: boolean;
	is_default: boolean;
	default_sample_rate: number | null;
	default_channels: number | null;
	default_sample_format: string | null;
};
