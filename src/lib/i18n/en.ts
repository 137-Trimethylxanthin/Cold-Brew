export default {
	// Nav
	'nav.library': 'Library',
	'nav.player': 'Player',
	'nav.explore': 'Explore',
	'nav.settings': 'Settings',
	'nav.bottom_nav': 'Bottom navigation',
	'nav.primary': 'Primary',

	// Transport
	'transport.previous': 'Previous track',
	'transport.play': 'Play',
	'transport.pause': 'Pause',
	'transport.stop': 'Stop',
	'transport.next': 'Next track',
	'transport.volume': 'Volume',
	'transport.speed': 'Playback speed',
	'transport.compact': 'Compact mode',
	'transport.expand': 'Expand player',

	// Settings tabs
	'settings.general': 'General',
	'settings.accounts': 'Accounts',
	'settings.providers': 'Providers',
	'settings.dev': 'Dev',
	'settings.audio': 'Audio',
	'settings.library': 'Library',

	// Settings: General
	'settings.accent_color': 'Accent Color',
	'settings.accent_color_desc': 'Choose an accent color for the interface',
	'settings.layout_density': 'Layout Density',
	'settings.layout_density_desc': 'Control spacing throughout the interface',
	'settings.notifications': 'Notifications',
	'settings.notifications_desc': 'Show "Now Playing" notification when tracks change',
	'settings.notifications_label': 'Enable now-playing notifications',
	'settings.language': 'Language',
	'settings.language_desc': 'Interface display language',
	'settings.high_contrast': 'High Contrast',
	'settings.high_contrast_desc': 'Increase contrast for better visibility',
	'settings.high_contrast_label': 'Enable high contrast mode',
	'settings.services': 'Services',
	'settings.services_desc': 'Provider capabilities and current implementation state',
	'settings.replaygain': 'ReplayGain',
	'settings.replaygain_desc':
		'Applied during local playback when matching ReplayGain tags are present',

	// Settings: Accounts
	'settings.jellyfin': 'Jellyfin',
	'settings.jellyfin_desc': 'Connect to your Jellyfin server',
	'settings.spotify': 'Spotify',
	'settings.spotify_desc': 'Connect your Spotify account',

	// Common UI
	'common.close': 'Close',
	'common.refresh': 'Refresh',
	'common.remove': 'Remove',
	'common.queue': 'Queue',
	'common.empty': 'Queue is empty',
	'common.up_next': 'Up next',
	'common.history': 'History',
	'common.now_playing': 'Now Playing',
	'common.fullscreen': 'Toggle fullscreen now-playing',
	'common.exit_fullscreen': 'Exit fullscreen',
	'common.offline': 'Offline — showing local files only',
	'common.back_online': 'Back online',
	'common.mode': 'Mode',
	'common.output': 'Output',
	'common.waiting_playback': 'Waiting for playback',
	'common.not_set': 'Not set',
	'common.connected': 'Connected',
	compact_label: 'Compact',
	comfortable_label: 'Comfortable',
	spacious_label: 'Spacious',

	// Color labels
	'color.cold_blue': 'Cold Blue',
	'color.caramel': 'Caramel',
	'color.rose': 'Rose',
	'color.mint': 'Mint',
	'color.lavender': 'Lavender',
	'color.amber': 'Amber',

	// Album art
	'album.art': '{title} album art'
} as const;
