<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type {
		LibraryTrack,
		ListeningHistoryEntry,
		ListeningHistorySummary,
		LyricsResult,
		MetadataSuggestion,
		PlaybackStatus,
		PlaylistDetail,
		PlaylistSummary,
		QueueSnapshot,
		RemotePlaylist,
		RemoteTrack,
		ScanSummary,
		Song
	} from '$lib/types';

	type SortKey = 'title' | 'artist' | 'album' | 'quality' | 'duration';
	type SortDirection = 'asc' | 'desc';
	type LocalColumn = 'artist' | 'album' | 'quality' | 'duration';
	type RemoteProvider = 'spotify' | 'tidal' | 'qobuz' | 'youtube' | 'lastfm';

	const localColumnOptions: { id: LocalColumn; label: string }[] = [
		{ id: 'artist', label: 'Artist' },
		{ id: 'album', label: 'Album' },
		{ id: 'quality', label: 'Quality' },
		{ id: 'duration', label: 'Time' }
	];
	const remoteProviderOptions: { id: RemoteProvider; label: string }[] = [
		{ id: 'spotify', label: 'Spotify' },
		{ id: 'tidal', label: 'TIDAL' },
		{ id: 'qobuz', label: 'Qobuz' },
		{ id: 'youtube', label: 'YouTube' },
		{ id: 'lastfm', label: 'Last.fm' }
	];

	let libraryPath = '';
	let playlistName = '';
	let playlistImportPath = '';
	let playlistExportPath = '';
	let localTracks: LibraryTrack[] = [];
	let jellyfinSongs: Song[] = [];
	let selectedRemoteProvider: RemoteProvider = 'spotify';
	let remoteQuery = '';
	let remoteCountryCode = 'US';
	let remoteResults: RemoteTrack[] = [];
	let remotePlaylists: RemotePlaylist[] = [];
	let selectedRemotePlaylistId: string | null = null;
	let playlists: PlaylistSummary[] = [];
	let listeningHistory: ListeningHistoryEntry[] = [];
	let listeningSummaries: ListeningHistorySummary[] = [];
	let selectedTrack: LibraryTrack | null = null;
	let selectedLyrics: LyricsResult | null = null;
	let metadataSuggestions: MetadataSuggestion[] = [];
	let loadingLyrics = false;
	let loadingMetadata = false;
	let selectedPlaylist: PlaylistDetail | null = null;
	let selectedPlaylistId: number | null = null;
	let sortKey: SortKey = 'title';
	let sortDirection: SortDirection = 'asc';
	let visibleColumns: Record<LocalColumn, boolean> = {
		artist: true,
		album: true,
		quality: true,
		duration: true
	};
	let loadingLibrary = false;
	let loadingJellyfin = false;
	let loadingRemote = false;
	let loadingRemotePlaylists = false;
	let message = '';
	let error = '';

	onMount(() => {
		void loadLocalLibrary();
		void loadPlaylists();
		void loadListeningHistory();
	});

	async function loadLocalLibrary() {
		loadingLibrary = true;
		error = '';
		try {
			localTracks = await invoke<LibraryTrack[]>('list_library_tracks');
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingLibrary = false;
		}
	}

	async function scanLibrary() {
		if (!libraryPath.trim()) {
			error = 'Enter a local music folder path.';
			return;
		}

		loadingLibrary = true;
		error = '';
		message = '';
		try {
			const summary = await invoke<ScanSummary>('scan_library_path', { path: libraryPath });
			localTracks = summary.tracks;
			message = `Indexed ${summary.indexed_tracks} of ${summary.scanned_files} audio files from ${summary.root}.`;
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingLibrary = false;
		}
	}

	async function loadJellyfin() {
		loadingJellyfin = true;
		error = '';
		try {
			const response = await invoke<unknown[]>('display_song_list');
			jellyfinSongs = response.map(toJellyfinSong);
			message = `Loaded ${jellyfinSongs.length} Jellyfin tracks.`;
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingJellyfin = false;
		}
	}

	async function searchRemote() {
		if (!remoteQuery.trim()) {
			error = `Enter a ${remoteProviderLabel(selectedRemoteProvider)} search query.`;
			return;
		}

		loadingRemote = true;
		error = '';
		message = '';
		try {
			remoteResults = await invoke<RemoteTrack[]>(remoteSearchCommand(selectedRemoteProvider), {
				query: remoteQuery,
				country_code: selectedRemoteProvider === 'tidal' ? remoteCountryCode : null,
				limit: 10
			});
			message = `Found ${remoteResults.length} ${remoteProviderLabel(selectedRemoteProvider)} tracks.`;
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingRemote = false;
		}
	}

	async function loadRemotePlaylists() {
		if (!remotePlaylistsSupported()) {
			error = 'Playlist loading is currently implemented for Spotify, TIDAL, and YouTube.';
			return;
		}
		if (selectedRemoteProvider === 'youtube' && !remoteQuery.trim()) {
			error = 'Enter a YouTube playlist search query.';
			return;
		}

		loadingRemotePlaylists = true;
		error = '';
		message = '';
		try {
			if (selectedRemoteProvider === 'spotify') {
				remotePlaylists = await invoke<RemotePlaylist[]>('list_spotify_playlists', {
					limit: 20
				});
				message = `Loaded ${remotePlaylists.length} Spotify playlists.`;
			} else if (selectedRemoteProvider === 'tidal') {
				if (remoteQuery.trim()) {
					remotePlaylists = await invoke<RemotePlaylist[]>('search_tidal_playlists', {
						query: remoteQuery,
						country_code: remoteCountryCode,
						limit: 10
					});
					message = `Found ${remotePlaylists.length} TIDAL playlists.`;
				} else {
					remotePlaylists = await invoke<RemotePlaylist[]>('list_tidal_playlists', {
						country_code: remoteCountryCode,
						limit: 20
					});
					message = `Loaded ${remotePlaylists.length} TIDAL playlists.`;
				}
			} else {
				remotePlaylists = await invoke<RemotePlaylist[]>('search_youtube_playlists', {
					query: remoteQuery,
					limit: 10
				});
				message = `Found ${remotePlaylists.length} YouTube playlists.`;
			}
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingRemotePlaylists = false;
		}
	}

	async function loadRemotePlaylistTracks(playlist: RemotePlaylist) {
		loadingRemote = true;
		error = '';
		message = '';
		try {
			selectedRemotePlaylistId = playlist.id;
			const command =
				playlist.source === 'youtube'
					? 'get_youtube_playlist_tracks'
					: playlist.source === 'tidal'
						? 'get_tidal_playlist_tracks'
						: 'get_spotify_playlist_tracks';
			const args: Record<string, unknown> = {
				playlist_id: playlist.id,
				limit: 50
			};
			if (playlist.source === 'tidal') args.country_code = remoteCountryCode;
			remoteResults = await invoke<RemoteTrack[]>(command, args);
			message = `Loaded ${remoteResults.length} tracks from ${playlist.name}.`;
		} catch (err) {
			error = toErrorMessage(err);
		} finally {
			loadingRemote = false;
		}
	}

	async function loadPlaylists() {
		error = '';
		try {
			playlists = await invoke<PlaylistSummary[]>('list_playlists');
			if (selectedPlaylistId !== null) {
				const stillExists = playlists.some((playlist) => playlist.id === selectedPlaylistId);
				if (stillExists) {
					selectedPlaylist = await invoke<PlaylistDetail>('get_playlist', {
						playlist_id: selectedPlaylistId
					});
				}
			}
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadListeningHistory() {
		error = '';
		try {
			const [history, summaries] = await Promise.all([
				invoke<ListeningHistoryEntry[]>('list_listening_history', { limit: 12 }),
				invoke<ListeningHistorySummary[]>('list_listening_history_summary', { limit: 8 })
			]);
			listeningHistory = history;
			listeningSummaries = summaries;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function createPlaylist() {
		if (!playlistName.trim()) {
			error = 'Enter a playlist name.';
			return;
		}

		error = '';
		message = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('create_playlist', { name: playlistName });
			selectedPlaylistId = selectedPlaylist.id;
			playlistName = '';
			await loadPlaylists();
			message = `Created playlist ${selectedPlaylist.name}.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function selectPlaylist(playlistId: number) {
		error = '';
		try {
			selectedPlaylistId = playlistId;
			selectedPlaylist = await invoke<PlaylistDetail>('get_playlist', { playlist_id: playlistId });
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function addLocalToPlaylist(track: LibraryTrack) {
		if (selectedPlaylistId === null) {
			error = 'Select or create a playlist first.';
			return;
		}

		error = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('add_song_to_playlist', {
				playlist_id: selectedPlaylistId,
				song: localTrackToSong(track)
			});
			await loadPlaylists();
			message = `Added ${track.title} to ${selectedPlaylist.name}.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function importPlaylist() {
		if (!playlistImportPath.trim()) {
			error = 'Enter an M3U or M3U8 path to import.';
			return;
		}

		error = '';
		message = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('import_m3u_playlist', {
				path: playlistImportPath,
				name: playlistName || null
			});
			selectedPlaylistId = selectedPlaylist.id;
			playlistName = '';
			await loadPlaylists();
			message = `Imported ${selectedPlaylist.tracks.length} tracks into ${selectedPlaylist.name}.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function exportPlaylist() {
		if (selectedPlaylistId === null) {
			error = 'Select a playlist to export.';
			return;
		}
		if (!playlistExportPath.trim()) {
			error = 'Enter an export path.';
			return;
		}

		error = '';
		message = '';
		try {
			await invoke('export_m3u_playlist', {
				playlist_id: selectedPlaylistId,
				path: playlistExportPath
			});
			message = 'Playlist exported.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function queueSong(song: Song) {
		error = '';
		try {
			await invoke<QueueSnapshot>('queue_song', { song });
			message = `Queued ${song.title}.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	function queueLocal(track: LibraryTrack) {
		void queueSong(localTrackToSong(track));
	}

	async function inspectTrack(track: LibraryTrack) {
		selectedTrack = track;
		selectedLyrics = null;
		metadataSuggestions = [];
		loadingLyrics = true;
		try {
			selectedLyrics = await invoke<LyricsResult | null>('get_track_lyrics', {
				path: track.path,
				title: track.title,
				artist: track.artist,
				album: track.album,
				duration_ms: track.duration_ms
			});
		} catch {
			selectedLyrics = null;
		} finally {
			loadingLyrics = false;
		}
	}

	async function lookupMetadata(track: LibraryTrack) {
		loadingMetadata = true;
		error = '';
		try {
			metadataSuggestions = await invoke<MetadataSuggestion[]>('search_metadata_suggestions', {
				title: track.title,
				artist: track.artist,
				album: track.album,
				duration_ms: track.duration_ms
			});
		} catch (err) {
			error = toErrorMessage(err);
			metadataSuggestions = [];
		} finally {
			loadingMetadata = false;
		}
	}

	function playSelectedTrack() {
		if (selectedTrack) void playLocal(selectedTrack);
	}

	function queueSelectedTrack() {
		if (selectedTrack) queueLocal(selectedTrack);
	}

	function addSelectedTrackToPlaylist() {
		if (selectedTrack) void addLocalToPlaylist(selectedTrack);
	}

	function lookupSelectedTrackMetadata() {
		if (selectedTrack) void lookupMetadata(selectedTrack);
	}

	async function playLocal(track: LibraryTrack) {
		error = '';
		try {
			const status = await invoke<PlaybackStatus>('play_local_track', {
				path: track.path,
				title: track.title
			});
			await loadListeningHistory();
			message = `Playing ${status.current_title ?? track.title}.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	function toJellyfinSong(element: any): Song {
		return {
			title: element.Name ?? 'Untitled',
			artist: element.Artists ? element.Artists.join(', ') : (element.Artist ?? 'Unknown artist'),
			album: element.Album ?? '',
			duration: element.RunTimeTicks ?? 0,
			id: element.Id ?? '',
			source: 'jellyfin',
			quality: 'remote library'
		};
	}

	function localTrackToSong(track: LibraryTrack): Song {
		return {
			id: track.path,
			title: track.title,
			artist: track.artist ?? 'Unknown artist',
			album: track.album ?? '',
			duration: Math.round((track.duration_ms ?? 0) * 10000),
			source: 'local',
			uri: track.path,
			quality: formatQuality(track) || track.extension.toUpperCase(),
			playable: true
		};
	}

	function remoteTrackToSong(track: RemoteTrack): Song {
		return {
			id: `${track.source}:${track.id}`,
			title: track.title,
			artist: track.artist || track.source,
			album: track.album ?? '',
			duration: Math.round((track.duration_ms ?? 0) * 10000),
			source: track.source,
			uri: track.uri,
			external_url: track.external_url,
			quality: track.quality ?? (track.playable ? 'remote playable' : 'metadata only'),
			playable: track.playable
		};
	}

	function queueRemote(track: RemoteTrack) {
		void queueSong(remoteTrackToSong(track));
	}

	function remoteSearchCommand(provider: RemoteProvider) {
		if (provider === 'spotify') return 'search_spotify_tracks';
		if (provider === 'tidal') return 'search_tidal_tracks';
		if (provider === 'qobuz') return 'search_qobuz_tracks';
		if (provider === 'youtube') return 'search_youtube_tracks';
		return 'search_lastfm_tracks';
	}

	function remoteProviderLabel(provider: RemoteProvider) {
		return remoteProviderOptions.find((option) => option.id === provider)?.label ?? provider;
	}

	function changeRemoteProvider() {
		remoteResults = [];
		remotePlaylists = [];
		selectedRemotePlaylistId = null;
		message = '';
		error = '';
	}

	function remotePlaylistsSupported() {
		return (
			selectedRemoteProvider === 'spotify' ||
			selectedRemoteProvider === 'tidal' ||
			selectedRemoteProvider === 'youtube'
		);
	}

	function remotePlaylistCountLabel(playlist: RemotePlaylist) {
		return playlist.track_count > 0 ? `${playlist.track_count} tracks` : 'playlist';
	}

	function sortLocalTracks(key: SortKey) {
		if (sortKey === key) {
			sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
			return;
		}
		sortKey = key;
		sortDirection = 'asc';
	}

	function toggleColumn(column: LocalColumn) {
		visibleColumns = { ...visibleColumns, [column]: !visibleColumns[column] };
	}

	function sortedLocalTracks() {
		return [...localTracks].sort((first, second) => {
			const result = compareTrackValues(first, second, sortKey);
			return sortDirection === 'asc' ? result : -result;
		});
	}

	function compareTrackValues(first: LibraryTrack, second: LibraryTrack, key: SortKey) {
		if (key === 'quality') return qualityScore(first) - qualityScore(second);
		if (key === 'duration') return (first.duration_ms ?? 0) - (second.duration_ms ?? 0);

		return textSortValue(first, key).localeCompare(textSortValue(second, key), undefined, {
			sensitivity: 'base',
			numeric: true
		});
	}

	function textSortValue(track: LibraryTrack, key: Exclude<SortKey, 'quality' | 'duration'>) {
		if (key === 'title') return track.title;
		if (key === 'artist') return track.artist ?? '';
		return track.album ?? '';
	}

	function qualityScore(track: LibraryTrack) {
		return (track.sample_rate ?? 0) * 1000 + (track.bit_depth ?? 0) * 10 + (track.bitrate ?? 0);
	}

	function sortIndicator(key: SortKey) {
		if (sortKey !== key) return '';
		return sortDirection === 'asc' ? 'Asc' : 'Desc';
	}

	function formatDuration(durationMs: number | null) {
		if (!durationMs) return '';
		const totalSeconds = Math.floor(durationMs / 1000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return `${minutes}:${seconds.toString().padStart(2, '0')}`;
	}

	function formatQuality(track: LibraryTrack) {
		const parts: string[] = [];
		if (track.sample_rate) parts.push(`${Math.round(track.sample_rate / 1000)} kHz`);
		if (track.bit_depth) parts.push(`${track.bit_depth}-bit`);
		if (track.bitrate) parts.push(`${track.bitrate} kbps`);
		return parts.join(' / ');
	}

	function formatFileSize(bytes: number) {
		if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function formatDate(seconds: number | null) {
		if (!seconds) return '';
		return new Date(seconds * 1000).toLocaleString();
	}

	function formatHistoryLabel(value: string | null) {
		if (!value) return '';
		return value.charAt(0).toUpperCase() + value.slice(1);
	}

	function toErrorMessage(err: unknown) {
		const message = typeof err === 'string' ? err : err instanceof Error ? err.message : null;
		if (message?.includes('__TAURI_INTERNALS__')) return '';
		if (message) return message;
		return 'Unexpected application error.';
	}
</script>

<section class="toolbar">
	<div>
		<h1>Library</h1>
		<p>{localTracks.length} local tracks indexed</p>
	</div>
	<div class="scan">
		<input bind:value={libraryPath} placeholder="/path/to/music" aria-label="Music folder path" />
		<button onclick={scanLibrary} disabled={loadingLibrary}>Scan</button>
		<button onclick={loadLocalLibrary} disabled={loadingLibrary}>Refresh</button>
	</div>
</section>

{#if error}
	<p class="error">{error}</p>
{/if}
{#if message}
	<p class="message">{message}</p>
{/if}

<section class="table-section">
	<div class="section-title">
		<div>
			<h2>Local Files</h2>
			<span>{loadingLibrary ? 'Scanning...' : 'Ready'}</span>
		</div>
		<div class="column-controls" aria-label="Visible local columns">
			{#each localColumnOptions as column}
				<label>
					<input
						type="checkbox"
						checked={visibleColumns[column.id]}
						onchange={() => toggleColumn(column.id)}
					/>
					{column.label}
				</label>
			{/each}
		</div>
	</div>
	<table>
		<thead>
			<tr>
				<th
					><button class="sort-button" onclick={() => sortLocalTracks('title')}
						>Title {sortIndicator('title')}</button
					></th
				>
				{#if visibleColumns.artist}
					<th
						><button class="sort-button" onclick={() => sortLocalTracks('artist')}
							>Artist {sortIndicator('artist')}</button
						></th
					>
				{/if}
				{#if visibleColumns.album}
					<th
						><button class="sort-button" onclick={() => sortLocalTracks('album')}
							>Album {sortIndicator('album')}</button
						></th
					>
				{/if}
				{#if visibleColumns.quality}
					<th
						><button class="sort-button" onclick={() => sortLocalTracks('quality')}
							>Quality {sortIndicator('quality')}</button
						></th
					>
				{/if}
				{#if visibleColumns.duration}
					<th
						><button class="sort-button" onclick={() => sortLocalTracks('duration')}
							>Time {sortIndicator('duration')}</button
						></th
					>
				{/if}
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each sortedLocalTracks() as track}
				<tr>
					<td>
						<strong>{track.title}</strong>
						<span>{track.extension.toUpperCase()}{track.has_artwork ? ' / Art' : ''}</span>
					</td>
					{#if visibleColumns.artist}
						<td>{track.artist ?? ''}</td>
					{/if}
					{#if visibleColumns.album}
						<td>{track.album ?? ''}</td>
					{/if}
					{#if visibleColumns.quality}
						<td>{formatQuality(track)}</td>
					{/if}
					{#if visibleColumns.duration}
						<td>{formatDuration(track.duration_ms)}</td>
					{/if}
					<td>
						<div class="actions">
							<button onclick={() => playLocal(track)}>Play</button>
							<button onclick={() => queueLocal(track)}>Queue</button>
							<button onclick={() => addLocalToPlaylist(track)}>Add</button>
							<button onclick={() => inspectTrack(track)}>Info</button>
						</div>
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
</section>

{#if selectedTrack}
	<section class="track-inspector">
		<div class="section-title">
			<div>
				<h2>{selectedTrack.title}</h2>
				<span>{selectedTrack.artist ?? 'Unknown artist'}</span>
			</div>
			<div class="actions">
				<button onclick={playSelectedTrack}>Play</button>
				<button onclick={queueSelectedTrack}>Queue</button>
				<button onclick={addSelectedTrackToPlaylist}>Add</button>
				<button onclick={lookupSelectedTrackMetadata} disabled={loadingMetadata}>Metadata</button>
			</div>
		</div>

		<dl>
			<div>
				<dt>Album</dt>
				<dd>{selectedTrack.album ?? ''}</dd>
			</div>
			<div>
				<dt>Genre</dt>
				<dd>{selectedTrack.genre ?? ''}</dd>
			</div>
			<div>
				<dt>Track</dt>
				<dd>{selectedTrack.track_number ?? ''}</dd>
			</div>
			<div>
				<dt>Quality</dt>
				<dd>{formatQuality(selectedTrack) || selectedTrack.extension.toUpperCase()}</dd>
			</div>
			<div>
				<dt>Duration</dt>
				<dd>{formatDuration(selectedTrack.duration_ms)}</dd>
			</div>
			<div>
				<dt>File size</dt>
				<dd>{formatFileSize(selectedTrack.file_size)}</dd>
			</div>
			<div>
				<dt>Modified</dt>
				<dd>{formatDate(selectedTrack.modified_secs)}</dd>
			</div>
			<div class="wide">
				<dt>Path</dt>
				<dd>{selectedTrack.path}</dd>
			</div>
		</dl>

		<div class="lyrics-panel">
			<h3>Lyrics</h3>
			{#if loadingLyrics}
				<p>Loading lyrics</p>
			{:else if selectedLyrics}
				<p>{selectedLyrics.synced ? 'Synced' : 'Plain'} from {selectedLyrics.source}</p>
				<pre>{selectedLyrics.content}</pre>
			{:else}
				<p>No local lyrics found</p>
			{/if}
		</div>

		<div class="metadata-panel">
			<h3>Metadata Suggestions</h3>
			{#if loadingMetadata}
				<p>Searching MusicBrainz</p>
			{:else if metadataSuggestions.length > 0}
				<table>
					<thead>
						<tr>
							<th>Title</th>
							<th>Artist</th>
							<th>Album</th>
							<th>Date</th>
							<th>Score</th>
						</tr>
					</thead>
					<tbody>
						{#each metadataSuggestions as suggestion}
							<tr>
								<td>
									<strong>{suggestion.title}</strong>
									<span>{suggestion.recording_mbid}</span>
								</td>
								<td>{suggestion.artist}</td>
								<td>{suggestion.album ?? ''}</td>
								<td>{suggestion.first_release_date ?? ''}</td>
								<td>{suggestion.score ?? ''}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{:else}
				<p>No suggestions loaded</p>
			{/if}
		</div>
	</section>
{/if}

<section class="playlist-section">
	<div class="section-title">
		<div>
			<h2>Playlists</h2>
			<span>{playlists.length} saved</span>
		</div>
		<div class="playlist-tools">
			<input bind:value={playlistName} placeholder="Playlist name" aria-label="Playlist name" />
			<button onclick={createPlaylist}>Create</button>
		</div>
	</div>

	<div class="playlist-grid">
		<div class="playlist-list">
			{#if playlists.length === 0}
				<p>No playlists yet</p>
			{:else}
				{#each playlists as playlist}
					<button
						class:selected={playlist.id === selectedPlaylistId}
						onclick={() => selectPlaylist(playlist.id)}
					>
						<span>{playlist.name}</span>
						<small>{playlist.track_count} tracks</small>
					</button>
				{/each}
			{/if}
		</div>

		<div class="playlist-detail">
			<div class="playlist-tools">
				<input
					bind:value={playlistImportPath}
					placeholder="/path/list.m3u"
					aria-label="M3U import path"
				/>
				<button onclick={importPlaylist}>Import</button>
			</div>
			<div class="playlist-tools">
				<input
					bind:value={playlistExportPath}
					placeholder="/path/export.m3u8"
					aria-label="M3U export path"
				/>
				<button onclick={exportPlaylist} disabled={selectedPlaylistId === null}>Export</button>
			</div>

			{#if selectedPlaylist}
				<ol>
					{#each selectedPlaylist.tracks as song}
						<li>
							<span>{song.title}</span>
							<button onclick={() => queueSong(song)}>Queue</button>
						</li>
					{/each}
				</ol>
			{:else}
				<p>Select a playlist</p>
			{/if}
		</div>
	</div>
</section>

<section class="history-section">
	<div class="section-title">
		<h2>Recent Listening</h2>
		<button onclick={loadListeningHistory}>Refresh</button>
	</div>
	{#if listeningHistory.length === 0 && listeningSummaries.length === 0}
		<p>No listening history yet</p>
	{:else}
		{#if listeningSummaries.length > 0}
			<table>
				<thead>
					<tr>
						<th>Track</th>
						<th>Plays</th>
						<th>Completed</th>
						<th>Skipped</th>
						<th>Listened</th>
						<th>Last played</th>
					</tr>
				</thead>
				<tbody>
					{#each listeningSummaries as summary}
						<tr>
							<td>
								<strong>{summary.title ?? summary.path}</strong>
								<span>{summary.source}</span>
							</td>
							<td>{summary.play_count}</td>
							<td>{summary.completion_count}</td>
							<td>{summary.skip_count}</td>
							<td>{formatDuration(summary.total_listened_ms)}</td>
							<td>{summary.last_played_at}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		<table>
			<thead>
				<tr>
					<th>Track</th>
					<th>Event</th>
					<th>Class</th>
					<th>Position</th>
					<th>Listened</th>
					<th>Time</th>
				</tr>
			</thead>
			<tbody>
				{#each listeningHistory as entry}
					<tr>
						<td>
							<strong>{entry.title ?? entry.path}</strong>
							<span>{entry.source}</span>
						</td>
						<td>{formatHistoryLabel(entry.event)}</td>
						<td>{formatHistoryLabel(entry.classification)}</td>
						<td>{formatDuration(entry.position_ms)}</td>
						<td>{formatDuration(entry.listened_ms)}</td>
						<td>{entry.created_at}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</section>

<section class="table-section">
	<div class="section-title">
		<div>
			<h2>Remote Search</h2>
			<span
				>{loadingRemote || loadingRemotePlaylists
					? 'Loading...'
					: `${remoteProviderLabel(selectedRemoteProvider)} metadata`}</span
			>
		</div>
		<div class="scan">
			<select bind:value={selectedRemoteProvider} onchange={changeRemoteProvider}>
				{#each remoteProviderOptions as provider}
					<option value={provider.id}>{provider.label}</option>
				{/each}
			</select>
			<input
				bind:value={remoteQuery}
				placeholder="Track, artist, album"
				aria-label="Remote search"
			/>
			{#if selectedRemoteProvider === 'tidal'}
				<input
					class="country-input"
					bind:value={remoteCountryCode}
					placeholder="US"
					aria-label="TIDAL country code"
				/>
			{/if}
			<button onclick={searchRemote} disabled={loadingRemote}>Search</button>
			<button
				onclick={loadRemotePlaylists}
				disabled={loadingRemotePlaylists || !remotePlaylistsSupported()}
			>
				Playlists
			</button>
		</div>
	</div>
	{#if remotePlaylists.length > 0}
		<div class="remote-playlists">
			{#each remotePlaylists as playlist}
				<button
					class:selected={playlist.id === selectedRemotePlaylistId}
					onclick={() => loadRemotePlaylistTracks(playlist)}
				>
					<span>{playlist.name}</span>
					<small>{remotePlaylistCountLabel(playlist)}</small>
				</button>
			{/each}
		</div>
	{/if}
	<table>
		<thead>
			<tr>
				<th>Source</th>
				<th>Title</th>
				<th>Artist</th>
				<th>Album</th>
				<th>Quality</th>
				<th>Time</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each remoteResults as track}
				<tr>
					<td>{track.source.toUpperCase()}</td>
					<td>
						<strong>{track.title}</strong>
						<span>{track.external_url ?? track.uri}</span>
					</td>
					<td>{track.artist}</td>
					<td>{track.album ?? ''}</td>
					<td>{track.quality ?? (track.playable ? 'Remote playable' : 'Metadata only')}</td>
					<td>{formatDuration(track.duration_ms)}</td>
					<td><button onclick={() => queueRemote(track)}>Queue</button></td>
				</tr>
			{/each}
		</tbody>
	</table>
</section>

<section class="table-section">
	<div class="section-title">
		<h2>Jellyfin</h2>
		<button onclick={loadJellyfin} disabled={loadingJellyfin}>Load songs</button>
	</div>
	<table>
		<thead>
			<tr>
				<th>Title</th>
				<th>Artist</th>
				<th>Album</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each jellyfinSongs as song}
				<tr>
					<td><strong>{song.title}</strong></td>
					<td>{song.artist}</td>
					<td>{song.album}</td>
					<td><button onclick={() => queueSong(song)}>Queue</button></td>
				</tr>
			{/each}
		</tbody>
	</table>
</section>

<style>
	.toolbar,
	.section-title {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
	}

	h1,
	h2,
	h3,
	p {
		margin: 0;
	}

	h1 {
		font-family: var(--font-display);
		font-size: clamp(42px, 6vw, 76px);
		line-height: 0.94;
	}

	h2 {
		font-family: var(--font-display);
		font-size: clamp(22px, 2vw, 30px);
		line-height: 1.04;
	}

	h3 {
		font-size: 0.95rem;
	}

	.toolbar p,
	.section-title span,
	td span {
		color: var(--muted);
		font-size: 0.84rem;
	}

	.toolbar {
		position: relative;
		overflow: hidden;
		min-height: 250px;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background:
			linear-gradient(
				145deg,
				color-mix(in oklch, var(--surface) 92%, transparent),
				color-mix(in oklch, var(--surface-2) 58%, transparent)
			);
		box-shadow: var(--shadow);
		padding: clamp(22px, 5vw, 56px);
	}

	.toolbar::after {
		content: '';
		position: absolute;
		right: clamp(18px, 5vw, 64px);
		bottom: clamp(18px, 5vw, 54px);
		width: min(28vw, 250px);
		aspect-ratio: 1;
		border-radius: var(--radius-lg);
		background:
			radial-gradient(
				circle at 48% 48%,
				color-mix(in oklch, var(--fg) 28%, transparent) 0 12%,
				transparent 13%
			),
			conic-gradient(from 230deg, var(--fg), var(--accent), var(--accent-2), var(--surface-2), var(--fg));
		opacity: 0.22;
		pointer-events: none;
	}

	.toolbar > * {
		position: relative;
		z-index: 1;
	}

	.section-title > div:first-child {
		display: grid;
		gap: 3px;
	}

	.column-controls {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 8px 12px;
	}

	.column-controls label {
		display: flex;
		align-items: center;
		gap: 5px;
		color: var(--muted);
		font-size: 0.82rem;
	}

	.column-controls input {
		width: auto;
		min-width: 0;
		padding: 0;
	}

	.scan {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		flex: 1 1 340px;
		justify-content: flex-end;
		min-width: min(100%, 340px);
	}

	input,
	select {
		flex: 1;
		min-width: min(100%, 160px);
		border: 1px solid var(--border);
		border-radius: 999px;
		background: color-mix(in oklch, var(--surface) 88%, transparent);
		color: var(--fg);
		padding: 0.5rem 0.65rem;
	}

	input::placeholder {
		color: color-mix(in oklch, var(--muted) 72%, transparent);
	}

	.country-input {
		flex: 0 0 72px;
		min-width: 72px;
		text-transform: uppercase;
	}

	.table-section {
		margin-top: 24px;
	}

	.playlist-section {
		margin-top: 24px;
	}

	.history-section {
		margin-top: 24px;
	}

	.track-inspector {
		margin-top: 18px;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 90%, transparent);
		padding: 18px;
	}

	.track-inspector dl {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 12px 16px;
		margin: 14px 0 0;
	}

	.track-inspector dl div {
		min-width: 0;
	}

	.track-inspector .wide {
		grid-column: 1 / -1;
	}

	dt {
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.76rem;
		text-transform: uppercase;
	}

	dd {
		margin: 3px 0 0;
		overflow-wrap: anywhere;
	}

	.lyrics-panel,
	.metadata-panel {
		display: grid;
		gap: 8px;
		margin-top: 14px;
		border-top: 1px solid var(--border);
		padding-top: 12px;
	}

	.lyrics-panel pre {
		max-height: 220px;
		overflow: auto;
		margin: 0;
		white-space: pre-wrap;
		font:
			0.84rem/1.45 ui-monospace,
			SFMono-Regular,
			Menlo,
			monospace;
	}

	.playlist-tools {
		display: flex;
		gap: 8px;
		min-width: min(100%, 440px);
	}

	.playlist-grid {
		display: grid;
		grid-template-columns: minmax(180px, 260px) minmax(0, 1fr);
		gap: 14px;
		margin-top: 10px;
	}

	.playlist-list,
	.playlist-detail {
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 10px;
	}

	.playlist-list {
		display: grid;
		align-content: start;
		gap: 6px;
	}

	.playlist-list button {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		text-align: left;
	}

	.playlist-list button.selected {
		background: color-mix(in oklch, var(--success) 14%, var(--surface));
		border-color: color-mix(in oklch, var(--success) 48%, var(--border));
	}

	.playlist-list small {
		color: var(--muted);
		white-space: nowrap;
	}

	.playlist-detail {
		display: grid;
		gap: 10px;
	}

	.remote-playlists {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 10px;
	}

	.remote-playlists button {
		display: grid;
		gap: 2px;
		min-width: 150px;
		text-align: left;
	}

	.remote-playlists button.selected {
		background: color-mix(in oklch, var(--success) 14%, var(--surface));
		border-color: color-mix(in oklch, var(--success) 48%, var(--border));
	}

	.remote-playlists small {
		color: var(--muted);
	}

	.playlist-detail ol {
		margin: 0;
		padding-left: 20px;
	}

	.playlist-detail li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 5px 0;
	}

	table {
		width: 100%;
		margin-top: 10px;
		border-collapse: collapse;
		background: color-mix(in oklch, var(--surface) 84%, transparent);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		overflow: hidden;
	}

	th,
	td {
		border-bottom: 1px solid var(--border);
		padding: 0.6rem 0.7rem;
		text-align: left;
		vertical-align: middle;
	}

	th {
		background: color-mix(in oklch, var(--surface-2) 62%, transparent);
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.78rem;
		text-transform: uppercase;
	}

	.sort-button {
		border: 0;
		background: transparent;
		color: inherit;
		padding: 0;
		text-transform: inherit;
		font-size: inherit;
		font-weight: 700;
	}

	.sort-button:hover {
		background: transparent;
		color: var(--accent);
		text-decoration: none;
	}

	td:first-child {
		display: grid;
		gap: 2px;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		white-space: nowrap;
	}

	.table-section,
	.playlist-section,
	.history-section {
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 90%, transparent);
		padding: 18px;
	}

	.error,
	.message {
		margin-top: 12px;
		padding: 0.65rem 0.8rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}

	.error {
		background: color-mix(in oklch, var(--danger) 20%, var(--surface));
		color: color-mix(in oklch, var(--danger) 72%, var(--fg));
	}

	.message {
		background: color-mix(in oklch, var(--success) 18%, var(--surface));
		color: color-mix(in oklch, var(--success) 82%, var(--fg));
	}

	@media (max-width: 1180px) and (min-width: 761px) {
		.toolbar {
			align-items: flex-end;
			min-height: 340px;
			padding: clamp(24px, 5vw, 48px);
		}

		.toolbar::after {
			width: min(34vw, 260px);
		}

		.playlist-grid {
			grid-template-columns: minmax(210px, 0.42fr) minmax(0, 1fr);
		}

		.track-inspector dl {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 760px) {
		.toolbar {
			align-items: stretch;
			flex-direction: column;
			min-height: 320px;
			padding: 22px;
		}

		.toolbar::after {
			right: 18px;
			bottom: 18px;
			width: min(64vw, 250px);
			opacity: 0.16;
		}

		h1 {
			font-size: clamp(38px, 12vw, 58px);
		}

		.section-title {
			align-items: flex-start;
		}

		.scan {
			flex-wrap: wrap;
			flex: 0 1 auto;
			justify-content: flex-start;
		}

		.column-controls {
			justify-content: flex-start;
		}

		.playlist-tools {
			flex-direction: column;
		}

		.playlist-grid {
			grid-template-columns: 1fr;
		}

		.track-inspector dl {
			grid-template-columns: 1fr 1fr;
		}

		.table-section,
		.playlist-section,
		.history-section {
			margin-top: 14px;
			padding: 14px;
		}

		.actions button {
			min-height: 34px;
			padding: 0 0.6rem;
		}

		table {
			display: block;
			overflow-x: auto;
		}
	}

	@media (max-width: 520px) {
		.track-inspector dl {
			grid-template-columns: 1fr;
		}

		th,
		td {
			padding: 0.55rem 0.6rem;
		}

		.remote-playlists button {
			min-width: 100%;
		}
	}
</style>
