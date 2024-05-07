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





</script>

{#await songsPromise}
    <p>Loading...</p>
{:then songs}
    <ul>
        {#each songs as song}
            <button>{song.Name}</button> <br>
        {/each}
    </ul>
{:catch error}
    <p>{error.message}</p>
{/await}

