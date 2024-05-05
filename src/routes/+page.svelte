<script>
	import { invoke } from "@tauri-apps/api/core";

    console.log('Hello from the server!');

    let songsPromise = invoke('display_song_list').then((res) => {
        console.log(res);
        return res;
    }).catch((err) => {
        console.log(err);
        return err;
    })

    /**
	 * @param {any} song
	 */
    async function addSong(song) {
        console.log(song);
        let res = await invoke('add_song', {song: song, id: "test"});
        console.log(res);
    }
</script>

{#await songsPromise}
    <p>Loading...</p>
{:then songs}
    <ul>
        {#each songs as song}
            <button on:click={() => addSong(song)}>{song.Name}</button> <br>
        {/each}
    </ul>
{:catch error}
    <p>{error.message}</p>
{/await}

