<script>
	import { invoke } from "@tauri-apps/api/core";

    console.log('Hello from the server!');

    let songs;

    async function loadSongs() {
        songs = await invoke('display_song_list');
        console.log(songs);
    }
    loadSongs();
</script>

<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://kit.svelte.dev">kit.svelte.dev</a> to read the documentation</p>
