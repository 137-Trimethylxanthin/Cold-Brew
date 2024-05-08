<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
    import { wsh } from "$lib/websocket";
    import type { Song } from "$lib/types";



    let songsPromise:Promise<Song[]> = invoke('display_song_list').then((res) => {
        console.log(res);
        console.log(typeof res);
        //convert the array artists to a string
        //go thro every element in display_song_list and create a song object
        let songs:Song[] = [];
        if (typeof res == "object" && res !== null){
            if (Array.isArray(res)){
                res.forEach((element: any) => {
                    songs.push({
                        title: element.Name,
                        artist: element.Artists ? element.Artists.join(", ") : element.Artist,
                        album: element.Album,
                        duration: element.RunTimeTicks / 10000000,
                        id: element.Id
                    });
                });
            }
        }
        return songs;
        //return songs;
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
            <button on:click={() => wsh.sendSong(song)} >{song.title}</button> <br>
        {/each}
    </ul>
{:catch error}
    <p>{error.message}</p>
{/await}

