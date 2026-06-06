import type en from './en';

const de: Record<keyof typeof en, string> = {
	// Nav
	'nav.library': 'Bibliothek',
	'nav.player': 'Player',
	'nav.explore': 'Entdecken',
	'nav.settings': 'Einstellungen',
	'nav.bottom_nav': 'Untere Navigation',
	'nav.primary': 'Primär',

	// Transport
	'transport.previous': 'Vorheriger Titel',
	'transport.play': 'Wiedergabe',
	'transport.pause': 'Pause',
	'transport.stop': 'Stopp',
	'transport.next': 'Nächster Titel',
	'transport.volume': 'Lautstärke',
	'transport.speed': 'Wiedergabegeschwindigkeit',
	'transport.compact': 'Kompaktmodus',
	'transport.expand': 'Player erweitern',

	// Settings tabs
	'settings.general': 'Allgemein',
	'settings.accounts': 'Konten',
	'settings.providers': 'Anbieter',
	'settings.dev': 'Entwicklung',
	'settings.audio': 'Audio',
	'settings.library': 'Bibliothek',

	// Settings: General
	'settings.accent_color': 'Akzentfarbe',
	'settings.accent_color_desc': 'Wähle eine Akzentfarbe für die Oberfläche',
	'settings.layout_density': 'Layout-Dichte',
	'settings.layout_density_desc': 'Steuere die Abstände in der Oberfläche',
	'settings.notifications': 'Benachrichtigungen',
	'settings.notifications_desc': '"Jetzt läuft"-Benachrichtigung bei Titelwechsel anzeigen',
	'settings.notifications_label': 'Jetzt-läuft-Benachrichtigungen aktivieren',
	'settings.language': 'Sprache',
	'settings.language_desc': 'Anzeigesprache der Oberfläche',
	'settings.high_contrast': 'Hoher Kontrast',
	'settings.high_contrast_desc': 'Kontrast für bessere Sichtbarkeit erhöhen',
	'settings.high_contrast_label': 'Hohen Kontrast aktivieren',
	'settings.services': 'Dienste',
	'settings.services_desc': 'Anbieter-Fähigkeiten und aktueller Implementierungsstatus',
	'settings.replaygain': 'ReplayGain',
	'settings.replaygain_desc':
		'Wird bei lokaler Wiedergabe angewendet, wenn passende ReplayGain-Tags vorhanden sind',

	// Settings: Accounts
	'settings.jellyfin': 'Jellyfin',
	'settings.jellyfin_desc': 'Mit deinem Jellyfin-Server verbinden',
	'settings.spotify': 'Spotify',
	'settings.spotify_desc': 'Dein Spotify-Konto verbinden',

	// Common UI
	'common.close': 'Schließen',
	'common.refresh': 'Aktualisieren',
	'common.remove': 'Entfernen',
	'common.queue': 'Warteschlange',
	'common.empty': 'Warteschlange ist leer',
	'common.up_next': 'Als nächstes',
	'common.history': 'Verlauf',
	'common.now_playing': 'Jetzt läuft',
	'common.fullscreen': 'Vollbildansicht umschalten',
	'common.exit_fullscreen': 'Vollbild verlassen',
	'common.offline': 'Offline — nur lokale Dateien',
	'common.back_online': 'Wieder online',
	'common.mode': 'Modus',
	'common.output': 'Ausgabe',
	'common.waiting_playback': 'Warte auf Wiedergabe',
	'common.not_set': 'Nicht gesetzt',
	'common.connected': 'Verbunden',
	compact_label: 'Kompakt',
	comfortable_label: 'Komfortabel',
	spacious_label: 'Großzügig',

	// Color labels
	'color.cold_blue': 'Kaltblau',
	'color.caramel': 'Karamell',
	'color.rose': 'Rose',
	'color.mint': 'Minze',
	'color.lavender': 'Lavendel',
	'color.amber': 'Bernstein',

	// Album art
	'album.art': '{title} Albumcover'
};

export default de;
