<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import * as Sheet from '$lib/components/ui/sheet';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import { Play, ListPlus, Plus, Info } from '@lucide/svelte';
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

	let playlistName = $state('');
	let playlistImportPath = $state('');
	let playlistExportPath = $state('');
	let localTracks: LibraryTrack[] = $state([]);
	let jellyfinSongs: Song[] = $state([]);
	let selectedRemoteProvider: RemoteProvider = $state('spotify');
	let remoteQuery = $state('');
	let remoteCountryCode = $state('US');
	let remoteResults: RemoteTrack[] = $state([]);
	let remotePlaylists: RemotePlaylist[] = $state([]);
	let selectedRemotePlaylistId: string | null = $state(null);
	let playlists: PlaylistSummary[] = $state([]);
	let listeningHistory: ListeningHistoryEntry[] = $state([]);
	let listeningSummaries: ListeningHistorySummary[] = $state([]);
	let selectedTrack: LibraryTrack | null = $state(null);
	let selectedLyrics: LyricsResult | null = $state(null);
	let metadataSuggestions: MetadataSuggestion[] = $state([]);
	let loadingLyrics = $state(false);
	let loadingMetadata = $state(false);
	let selectedPlaylist: PlaylistDetail | null = $state(null);
	let selectedPlaylistId: number | null = $state(null);
	let trackInspectorOpen = $state(false);
	let sortKey: SortKey = $state('title');
	let sortDirection: SortDirection = $state('asc');
	let visibleColumns: Record<LocalColumn, boolean> = $state({
		artist: true,
		album: true,
		quality: true,
		duration: true
	});
	let loadingLibrary = $state(false);
	let loadingJellyfin = $state(false);
	let loadingRemote = $state(false);
	let loadingRemotePlaylists = $state(false);
	let message = $state('');
	let error = $state('');

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
				remotePlaylists = await invoke<RemotePlaylist[]>('list_spotify_playlists', { limit: 20 });
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
				const stillExists = playlists.some((p) => p.id === selectedPlaylistId);
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
		if (!playlistName.trim()) { error = 'Enter a playlist name.'; return; }
		error = ''; message = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('create_playlist', { name: playlistName });
			selectedPlaylistId = selectedPlaylist.id;
			playlistName = '';
			await loadPlaylists();
			message = `Created playlist ${selectedPlaylist.name}.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function selectPlaylist(playlistId: number) {
		error = '';
		try {
			selectedPlaylistId = playlistId;
			selectedPlaylist = await invoke<PlaylistDetail>('get_playlist', { playlist_id: playlistId });
		} catch (err) { error = toErrorMessage(err); }
	}

	async function addLocalToPlaylist(track: LibraryTrack) {
		if (selectedPlaylistId === null) { error = 'Select or create a playlist first.'; return; }
		error = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('add_song_to_playlist', {
				playlist_id: selectedPlaylistId,
				song: localTrackToSong(track)
			});
			await loadPlaylists();
			message = `Added ${track.title} to ${selectedPlaylist.name}.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function importPlaylist() {
		if (!playlistImportPath.trim()) { error = 'Enter an M3U or M3U8 path to import.'; return; }
		error = ''; message = '';
		try {
			selectedPlaylist = await invoke<PlaylistDetail>('import_m3u_playlist', {
				path: playlistImportPath, name: playlistName || null
			});
			selectedPlaylistId = selectedPlaylist.id;
			playlistName = '';
			await loadPlaylists();
			message = `Imported ${selectedPlaylist.tracks.length} tracks into ${selectedPlaylist.name}.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function exportPlaylist() {
		if (selectedPlaylistId === null) { error = 'Select a playlist to export.'; return; }
		if (!playlistExportPath.trim()) { error = 'Enter an export path.'; return; }
		error = ''; message = '';
		try {
			await invoke('export_m3u_playlist', { playlist_id: selectedPlaylistId, path: playlistExportPath });
			message = 'Playlist exported.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function queueSong(song: Song) {
		error = '';
		try {
			await invoke<QueueSnapshot>('queue_song', { song });
			message = `Queued ${song.title}.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	function queueLocal(track: LibraryTrack) { void queueSong(localTrackToSong(track)); }

	async function inspectTrack(track: LibraryTrack) {
		selectedTrack = track;
		trackInspectorOpen = true;
		selectedLyrics = null; metadataSuggestions = []; loadingLyrics = true;
		try {
			selectedLyrics = await invoke<LyricsResult | null>('get_track_lyrics', {
				path: track.path, title: track.title, artist: track.artist,
				album: track.album, duration_ms: track.duration_ms
			});
		} catch { selectedLyrics = null; }
		finally { loadingLyrics = false; }
	}

	async function lookupMetadata(track: LibraryTrack) {
		loadingMetadata = true; error = '';
		try {
			metadataSuggestions = await invoke<MetadataSuggestion[]>('search_metadata_suggestions', {
				title: track.title, artist: track.artist, album: track.album, duration_ms: track.duration_ms
			});
		} catch (err) { error = toErrorMessage(err); metadataSuggestions = []; }
		finally { loadingMetadata = false; }
	}

	function playSelectedTrack() { if (selectedTrack) void playLocal(selectedTrack); }
	function queueSelectedTrack() { if (selectedTrack) queueLocal(selectedTrack); }
	function addSelectedTrackToPlaylist() { if (selectedTrack) void addLocalToPlaylist(selectedTrack); }
	function lookupSelectedTrackMetadata() { if (selectedTrack) void lookupMetadata(selectedTrack); }

	async function playLocal(track: LibraryTrack) {
		error = '';
		try {
			await invoke('play_track_now', { song: localTrackToSong(track) });
			await loadListeningHistory();
			message = `Playing ${track.title}.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	function toJellyfinSong(element: any): Song {
		return {
			title: element.Name ?? 'Untitled',
			artist: element.Artists ? element.Artists.join(', ') : (element.Artist ?? 'Unknown artist'),
			album: element.Album ?? '', duration: element.RunTimeTicks ?? 0,
			id: element.Id ?? '', source: 'jellyfin', quality: 'remote library'
		};
	}

	function localTrackToSong(track: LibraryTrack): Song {
		return {
			id: track.path, title: track.title,
			artist: track.artist ?? 'Unknown artist', album: track.album ?? '',
			duration: Math.round((track.duration_ms ?? 0) * 10000),
			source: 'local', uri: track.path,
			quality: formatQuality(track) || track.extension.toUpperCase(), playable: true
		};
	}

	function remoteTrackToSong(track: RemoteTrack): Song {
		return {
			id: `${track.source}:${track.id}`, title: track.title,
			artist: track.artist || track.source, album: track.album ?? '',
			duration: Math.round((track.duration_ms ?? 0) * 10000),
			source: track.source, uri: track.uri, external_url: track.external_url,
			quality: track.quality ?? (track.playable ? 'remote playable' : 'metadata only'),
			playable: track.playable
		};
	}

	function queueRemote(track: RemoteTrack) { void queueSong(remoteTrackToSong(track)); }

	function remoteSearchCommand(provider: RemoteProvider) {
		if (provider === 'spotify') return 'search_spotify_tracks';
		if (provider === 'tidal') return 'search_tidal_tracks';
		if (provider === 'qobuz') return 'search_qobuz_tracks';
		if (provider === 'youtube') return 'search_youtube_tracks';
		return 'search_lastfm_tracks';
	}

	function remoteProviderLabel(provider: RemoteProvider) {
		return remoteProviderOptions.find((o) => o.id === provider)?.label ?? provider;
	}

	function changeRemoteProvider() {
		remoteResults = []; remotePlaylists = []; selectedRemotePlaylistId = null;
		message = ''; error = '';
	}

	function remotePlaylistsSupported() {
		return selectedRemoteProvider === 'spotify' || selectedRemoteProvider === 'tidal' || selectedRemoteProvider === 'youtube';
	}

	function remotePlaylistCountLabel(playlist: RemotePlaylist) {
		return playlist.track_count > 0 ? `${playlist.track_count} tracks` : 'playlist';
	}

	function sortLocalTracks(key: SortKey) {
		if (sortKey === key) { sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'; return; }
		sortKey = key; sortDirection = 'asc';
	}

	function toggleColumn(column: LocalColumn) {
		visibleColumns = { ...visibleColumns, [column]: !visibleColumns[column] };
	}

	function sortedLocalTracks() {
		return [...localTracks].sort((a, b) => {
			const result = compareTrackValues(a, b, sortKey);
			return sortDirection === 'asc' ? result : -result;
		});
	}

	function compareTrackValues(a: LibraryTrack, b: LibraryTrack, key: SortKey) {
		if (key === 'quality') return qualityScore(a) - qualityScore(b);
		if (key === 'duration') return (a.duration_ms ?? 0) - (b.duration_ms ?? 0);
		return textSortValue(a, key).localeCompare(textSortValue(b, key), undefined, { sensitivity: 'base', numeric: true });
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

<section class="heading-bg relative overflow-hidden border border-border rounded-3xl shadow-2xl p-[clamp(22px,5vw,52px)]"
	style="min-height: 210px">
	<div class="relative z-[1]">
		<h1 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(42px,6vw,76px)] leading-[0.94]">Library</h1>
		<p class="text-muted text-sm">{localTracks.length} local tracks indexed — scan new paths in Settings</p>
	</div>
	<div class="flex gap-2 relative z-[1]">
		<Button variant="secondary" size="sm" onclick={loadLocalLibrary} disabled={loadingLibrary}>Refresh</Button>
	</div>
	<div class="hero-blob absolute right-[clamp(18px,5vw,64px)] bottom-[clamp(18px,5vw,54px)] w-[min(28vw,250px)] aspect-square rounded-3xl opacity-[0.22] pointer-events-none"></div>
</section>

{#if error}<p class="mt-3 px-3.5 py-2.5 border border-border rounded-[20px] bg-danger/20 text-danger/70">{error}</p>{/if}
{#if message}<p class="mt-3 px-3.5 py-2.5 border border-border rounded-[20px] bg-success/20 text-success/80">{message}</p>{/if}

<section class="mt-6 border border-border rounded-3xl p-[18px] bg-surface/90">
	<div class="flex items-center justify-between gap-3 flex-wrap mb-2">
		<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Local Files</h2>
		<div class="flex justify-center items-center gap-1.5 mb-3">
			{#each localColumnOptions as column}
				<Button variant={visibleColumns[column.id] ? 'default' : 'outline'} size="sm" onclick={() => toggleColumn(column.id)}>
					{column.label}
				</Button>
			{/each}
		</div>
	</div>
	<div class="overflow-auto rounded-[20px] border border-border">
	<Table.Root class="w-full table-fixed">
		<Table.Header>
			<Table.Row>
				<Table.Head class="w-2/5 h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">
					<Button variant="ghost" size="sm" class="-mx-2 truncate max-w-full" onclick={() => sortLocalTracks('title')}>
						Title {sortIndicator('title')}
					</Button>
				</Table.Head>
				{#if visibleColumns.artist}
					<Table.Head class="w-1/5 h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">
						<Button variant="ghost" size="sm" class="-mx-2 truncate max-w-full" onclick={() => sortLocalTracks('artist')}>
							Artist {sortIndicator('artist')}
						</Button>
					</Table.Head>
				{/if}
				{#if visibleColumns.album}
					<Table.Head class="w-1/5 h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">
						<Button variant="ghost" size="sm" class="-mx-2 truncate max-w-full" onclick={() => sortLocalTracks('album')}>
							Album {sortIndicator('album')}
						</Button>
					</Table.Head>
				{/if}
				{#if visibleColumns.quality}
					<Table.Head class="w-[90px] h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">
						<Button variant="ghost" size="sm" class="-mx-2" onclick={() => sortLocalTracks('quality')}>
							Quality {sortIndicator('quality')}
						</Button>
					</Table.Head>
				{/if}
				{#if visibleColumns.duration}
					<Table.Head class="w-[70px] h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">
						<Button variant="ghost" size="sm" class="-mx-2" onclick={() => sortLocalTracks('duration')}>
							Time {sortIndicator('duration')}
						</Button>
					</Table.Head>
				{/if}
				<Table.Head class="w-[140px] h-10 px-2 text-center align-middle text-muted font-mono text-xs uppercase tracking-wider">Actions</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each sortedLocalTracks().slice(0, 100) as track}
				<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
					<Table.Cell class="px-3 py-2 align-middle min-w-0">
						<div class="truncate">
							<strong class="text-sm">{track.title}</strong>
							<span class="text-muted text-xs ml-1.5">{track.extension.toUpperCase()}{track.has_artwork ? ' · Art' : ''}</span>
						</div>
					</Table.Cell>
					{#if visibleColumns.artist}<Table.Cell class="px-3 py-2 align-middle min-w-0"><span class="truncate block text-sm">{track.artist ?? '—'}</span></Table.Cell>{/if}
					{#if visibleColumns.album}<Table.Cell class="px-3 py-2 align-middle min-w-0"><span class="truncate block text-sm">{track.album ?? '—'}</span></Table.Cell>{/if}
					{#if visibleColumns.quality}<Table.Cell class="px-3 py-2 align-middle text-sm text-muted font-mono">{formatQuality(track)}</Table.Cell>{/if}
					{#if visibleColumns.duration}<Table.Cell class="px-3 py-2 align-middle text-sm text-muted font-mono tabular-nums">{formatDuration(track.duration_ms)}</Table.Cell>{/if}
					<Table.Cell class="px-1.5 py-1.5 align-middle">
						<div class="flex justify-center gap-1">
							<Button size="sm" class="h-7 w-7 p-0" onclick={() => playLocal(track)} aria-label="Play"><Play class="size-3.5" /></Button>
							<Button variant="outline" size="sm" class="h-7 w-7 p-0" onclick={() => queueLocal(track)} aria-label="Queue"><ListPlus class="size-3.5" /></Button>
							<Button variant="secondary" size="sm" class="h-7 w-7 p-0" onclick={() => addLocalToPlaylist(track)} aria-label="Add to playlist"><Plus class="size-3.5" /></Button>
							<Button variant="outline" size="sm" class="h-7 w-7 p-0" onclick={() => inspectTrack(track)} aria-label="Track info"><Info class="size-3.5" /></Button>
						</div>
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
	</div>

</section>

<Sheet.Sheet bind:open={trackInspectorOpen}>
	<Sheet.SheetContent side="right" class="w-[480px] sm:max-w-[480px]">
		<Sheet.SheetHeader>
			<Sheet.SheetTitle>{selectedTrack?.title ?? 'Track Info'}</Sheet.SheetTitle>
			{#if selectedTrack}
				<Sheet.SheetDescription>{selectedTrack.artist ?? 'Unknown artist'}</Sheet.SheetDescription>
			{/if}
		</Sheet.SheetHeader>

		{#if selectedTrack}
			<div class="mt-2.5">
				<div class="flex flex-wrap gap-1.5 whitespace-nowrap">
					<Button size="sm" class="h-7 px-2.5 text-xs" onclick={playSelectedTrack}>Play</Button>
					<Button variant="outline" size="sm" class="h-7 px-2.5 text-xs" onclick={queueSelectedTrack}>Queue</Button>
					<Button variant="secondary" size="sm" class="h-7 px-2.5 text-xs" onclick={addSelectedTrackToPlaylist}>Add</Button>
					<Button variant="outline" size="sm" class="h-7 px-2.5 text-xs" onclick={lookupSelectedTrackMetadata} disabled={loadingMetadata}>Metadata</Button>
				</div>

				<dl class="track-inspector-dl">
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Album</dt><dd class="mt-[3px] break-words">{selectedTrack.album ?? ''}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Genre</dt><dd class="mt-[3px] break-words">{selectedTrack.genre ?? ''}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Track</dt><dd class="mt-[3px] break-words">{selectedTrack.track_number ?? ''}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Quality</dt><dd class="mt-[3px] break-words">{formatQuality(selectedTrack) || selectedTrack.extension.toUpperCase()}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Duration</dt><dd class="mt-[3px] break-words">{formatDuration(selectedTrack.duration_ms)}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">File size</dt><dd class="mt-[3px] break-words">{formatFileSize(selectedTrack.file_size)}</dd></div>
					<div class="min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Modified</dt><dd class="mt-[3px] break-words">{formatDate(selectedTrack.modified_secs)}</dd></div>
					<div class="col-span-full min-w-0"><dt class="text-muted font-mono text-[0.76rem] uppercase">Path</dt><dd class="mt-[3px] break-words">{selectedTrack.path}</dd></div>
				</dl>

				<div class="grid gap-2 mt-3.5 pt-3 border-t border-border">
					<h3 class="text-[0.95rem]">Lyrics</h3>
					{#if loadingLyrics}
						<p>Loading lyrics</p>
					{:else if selectedLyrics}
						<p>{selectedLyrics.synced ? 'Synced' : 'Plain'} from {selectedLyrics.source}</p>
						<pre class="max-h-[220px] overflow-auto m-0 whitespace-pre-wrap text-[0.84rem]/[1.45] font-mono">{selectedLyrics.content}</pre>
					{:else}
						<p>No local lyrics found</p>
					{/if}
				</div>

				<div class="grid gap-2 mt-3.5 pt-3 border-t border-border">
					<h3 class="text-[0.95rem]">Metadata Suggestions</h3>
					{#if loadingMetadata}
						<p>Searching MusicBrainz</p>
					{:else if metadataSuggestions.length > 0}
						<Table.Root class="w-full rounded-[20px] overflow-hidden border border-border mt-2.5">
							<Table.Header>
								<Table.Row>
									<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Title</Table.Head>
									<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Artist</Table.Head>
									<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Album</Table.Head>
									<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Date</Table.Head>
									<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Score</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each metadataSuggestions as suggestion}
									<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
										<Table.Cell class="px-3 py-2.5 align-middle">
											<strong>{suggestion.title}</strong>
											<span class="text-muted text-sm block">{suggestion.recording_mbid}</span>
										</Table.Cell>
										<Table.Cell class="px-3 py-2.5 align-middle">{suggestion.artist}</Table.Cell>
										<Table.Cell class="px-3 py-2.5 align-middle">{suggestion.album ?? ''}</Table.Cell>
										<Table.Cell class="px-3 py-2.5 align-middle">{suggestion.first_release_date ?? ''}</Table.Cell>
										<Table.Cell class="px-3 py-2.5 align-middle">{suggestion.score ?? ''}</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					{:else}
						<p>No suggestions loaded</p>
					{/if}
				</div>
			</div>
		{/if}
	</Sheet.SheetContent>
</Sheet.Sheet>

<section class="mt-6 border border-border rounded-3xl p-[18px] bg-surface/90">
	<h2 class="m-0 mb-2 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Playlists</h2>
	<div class="library-playlist-grid">
		<div class="grid content-start gap-1.5 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
			<div class="flex gap-2 min-w-[min(100%,440px)]">
				<input bind:value={playlistName} placeholder="Playlist name" aria-label="Playlist name" class="flex-1 min-w-[min(100%,160px)] border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem] placeholder:text-muted/70" />
				<Button variant="default" size="sm" onclick={createPlaylist}>Create</Button>
			</div>
			{#if playlists.length === 0}
				<p>No playlists yet</p>
			{:else}
				{#each playlists as playlist}
				<Button
				variant={playlist.id === selectedPlaylistId ? 'default' : 'outline'}
				size="sm"
				class="flex justify-between gap-2 text-left w-full"
				onclick={() => selectPlaylist(playlist.id)}>
				 <span>{playlist.name}</span>
					<small class="text-muted whitespace-nowrap">{playlist.track_count} tracks</small>
				</Button>
				{/each}
			{/if}
		</div>

		<div class="grid gap-2.5 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
			<div class="flex gap-2 min-w-[min(100%,440px)]">
				<input bind:value={playlistImportPath} placeholder="/path/list.m3u" aria-label="M3U import path" class="flex-1 min-w-[min(100%,160px)] border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem] placeholder:text-muted/70" />
				<Button variant="default" size="sm" onclick={importPlaylist}>Import</Button>
			</div>
			<div class="flex gap-2 min-w-[min(100%,440px)]">
				<input bind:value={playlistExportPath} placeholder="/path/export.m3u8" aria-label="M3U export path" class="flex-1 min-w-[min(100%,160px)] border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem] placeholder:text-muted/70" />
				<Button variant="default" size="sm" onclick={exportPlaylist} disabled={selectedPlaylistId === null}>Export</Button>
			</div>

			{#if selectedPlaylist}
				<ol class="m-0 pl-5">
					{#each selectedPlaylist.tracks as song}
						<li class="flex items-center justify-between gap-2 py-[5px]">
							<span>{song.title}</span>
							<Button variant="outline" size="sm" class="h-7 px-2.5 text-xs" onclick={() => queueSong(song)}>Queue</Button>
						</li>
					{/each}
				</ol>
			{:else}
				<p>Select a playlist</p>
			{/if}
		</div>
	</div>
</section>

<section class="mt-6 border border-border rounded-3xl p-[18px] bg-surface/90">
	<h2 class="m-0 mb-2 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Recent Listening</h2>
	<div class="flex items-center justify-between gap-3 flex-wrap mb-2">
		<span class="text-muted text-sm">{listeningHistory.length} entries</span>
		<Button variant="secondary" size="sm" onclick={loadListeningHistory}>Refresh</Button>
	</div>
	{#if listeningHistory.length === 0 && listeningSummaries.length === 0}
		<p>No listening history yet</p>
	{:else}
		{#if listeningSummaries.length > 0}
			<Table.Root class="w-full rounded-[20px] overflow-hidden border border-border mt-3">
				<Table.Header>
					<Table.Row>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Track</Table.Head>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Plays</Table.Head>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Completed</Table.Head>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Skipped</Table.Head>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Listened</Table.Head>
						<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Last played</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each listeningSummaries as summary}
						<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
							<Table.Cell class="px-3 py-2.5 align-middle">
								<strong>{summary.title ?? summary.path}</strong>
								<span class="text-muted text-sm block">{summary.source}</span>
							</Table.Cell>
							<Table.Cell class="px-3 py-2.5 align-middle">{summary.play_count}</Table.Cell>
							<Table.Cell class="px-3 py-2.5 align-middle">{summary.completion_count}</Table.Cell>
							<Table.Cell class="px-3 py-2.5 align-middle">{summary.skip_count}</Table.Cell>
							<Table.Cell class="px-3 py-2.5 align-middle">{formatDuration(summary.total_listened_ms)}</Table.Cell>
							<Table.Cell class="px-3 py-2.5 align-middle">{summary.last_played_at}</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		{/if}
		<Table.Root class="w-full rounded-[20px] overflow-hidden border border-border mt-3">
			<Table.Header>
				<Table.Row>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Track</Table.Head>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Event</Table.Head>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Class</Table.Head>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Position</Table.Head>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Listened</Table.Head>
					<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Time</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each listeningHistory as entry}
					<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
						<Table.Cell class="px-3 py-2.5 align-middle">
							<strong>{entry.title ?? entry.path}</strong>
							<span class="text-muted text-sm block">{entry.source}</span>
						</Table.Cell>
						<Table.Cell class="px-3 py-2.5 align-middle">{formatHistoryLabel(entry.event)}</Table.Cell>
						<Table.Cell class="px-3 py-2.5 align-middle">{formatHistoryLabel(entry.classification)}</Table.Cell>
						<Table.Cell class="px-3 py-2.5 align-middle">{formatDuration(entry.position_ms)}</Table.Cell>
						<Table.Cell class="px-3 py-2.5 align-middle">{formatDuration(entry.listened_ms)}</Table.Cell>
						<Table.Cell class="px-3 py-2.5 align-middle">{entry.created_at}</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	{/if}
</section>

<section class="mt-6 border border-border rounded-3xl p-[18px] bg-surface/90">
	<h2 class="m-0 mb-2 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Remote Search</h2>
	<div class="flex items-center justify-between gap-3 flex-wrap mb-2">
		<span class="text-muted text-sm">{loadingRemote ? 'Loading...' : `${remoteProviderLabel(selectedRemoteProvider)} metadata`}</span>
	</div>
	<div class="library-scan-row">
		<select bind:value={selectedRemoteProvider} onchange={changeRemoteProvider} class="flex-1 min-w-[min(100%,160px)] border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem]">
			{#each remoteProviderOptions as provider}
				<option value={provider.id}>{provider.label}</option>
			{/each}
		</select>
		<input bind:value={remoteQuery} placeholder="Track, artist, album" aria-label="Remote search" class="flex-1 min-w-[min(100%,160px)] border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem] placeholder:text-muted/70" />
		{#if selectedRemoteProvider === 'tidal'}
			<input bind:value={remoteCountryCode} placeholder="US" aria-label="TIDAL country code" class="flex-[0_0_72px] min-w-[72px] uppercase border border-border rounded-full bg-surface/88 text-fg py-2 px-[0.65rem] placeholder:text-muted/70" />
		{/if}
		<Button variant="default" size="sm" onclick={searchRemote} disabled={loadingRemote}>Search</Button>
		<Button variant="outline" size="sm" onclick={loadRemotePlaylists} disabled={loadingRemotePlaylists || !remotePlaylistsSupported()}>Playlists</Button>
	</div>
	{#if remotePlaylists.length > 0}
		<div class="flex flex-wrap gap-2 mb-2">
			{#each remotePlaylists as playlist}
				<Button
					variant={playlist.id === selectedRemotePlaylistId ? 'default' : 'outline'}
					size="sm"
					class="grid gap-0.5 min-w-[150px] text-left h-auto py-2"
					onclick={() => loadRemotePlaylistTracks(playlist)}>
					<span>{playlist.name}</span>
					<small class="text-muted">{remotePlaylistCountLabel(playlist)}</small>
				</Button>
			{/each}
		</div>
	{/if}
	<Table.Root class="w-full rounded-[20px] overflow-hidden border border-border mt-3">
		<Table.Header>
			<Table.Row>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Source</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Title</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Artist</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Album</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Quality</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Time</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider"></Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each remoteResults as track}
				<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
					<Table.Cell class="px-3 py-2.5 align-middle">{track.source.toUpperCase()}</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">
						<strong>{track.title}</strong>
						<span class="text-muted text-sm block">{track.external_url ?? track.uri}</span>
					</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{track.artist}</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{track.album ?? ''}</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{track.quality ?? (track.playable ? 'Remote playable' : 'Metadata only')}</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{formatDuration(track.duration_ms)}</Table.Cell>
					<Table.Cell class="px-3 py-2 align-middle"><Button variant="outline" size="sm" class="h-7 px-2.5 text-xs" onclick={() => queueRemote(track)}>Queue</Button></Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</section>

<section class="mt-6 border border-border rounded-3xl p-[18px] bg-surface/90">
	<h2 class="m-0 mb-2 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Jellyfin</h2>
	<div class="flex items-center justify-between gap-3 flex-wrap mb-2">
		<span class="text-muted text-sm">Remote media server</span>
		<Button variant="default" size="sm" onclick={loadJellyfin} disabled={loadingJellyfin}>Load songs</Button>
	</div>
	<Table.Root class="w-full rounded-[20px] overflow-hidden border border-border mt-3">
		<Table.Header>
			<Table.Row>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Title</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Artist</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider">Album</Table.Head>
				<Table.Head class="h-10 px-3 text-left align-middle text-muted font-mono text-xs uppercase tracking-wider"></Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each jellyfinSongs as song}
				<Table.Row class="border-b border-border/60 hover:bg-surface/50 transition-colors">
					<Table.Cell class="px-3 py-2.5 align-middle"><strong>{song.title}</strong></Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{song.artist}</Table.Cell>
					<Table.Cell class="px-3 py-2.5 align-middle">{song.album}</Table.Cell>
					<Table.Cell class="px-3 py-2 align-middle"><Button variant="outline" size="sm" class="h-7 px-2.5 text-xs" onclick={() => queueSong(song)}>Queue</Button></Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</section>


