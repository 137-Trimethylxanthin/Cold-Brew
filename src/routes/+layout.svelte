<script lang="ts">
	import { goto } from '$app/navigation';
    import { invoke } from "@tauri-apps/api/core";
    import type { Song } from "$lib/types";


    let currentSong: Song = {title: "loading", artist: "loading", album: "loading", duration: 0, id: "loading"};
    let oldSongs: Song[] = [];
    let upcomingSongs: Song[] = [];

    console.log('Hello from the Layout!');

    async function getQueue() {
        currentSong = await invoke("get_current_song", {id:"test"});
        oldSongs = await invoke("old_queue", {id:"test"});
        upcomingSongs = await invoke("upcoming_queue", {id:"test"});
        console.log("currentSong");
        console.log(currentSong);
        console.log("oldSongs");
        console.log(oldSongs);
        console.log("upcomingSongs");
        console.log(upcomingSongs);


    }

    getQueue();


</script>

<!-- Side Nav -->

<div class="sidenav">
    <button on:click={() => goto("/")}>Home</button>
    <button on:click={() => goto("player")}>Player</button>
    <button on:click={getQueue}>reload Queue</button>
</div>

<slot/>

<div class="queue">
    <h1>Queue</h1>
    <h2>old</h2>
        <ul>
            {#each oldSongs as song}
                <li>{song.title}</li>
            {/each}
        </ul>
    <h2>current</h2>
    <h3>{currentSong.title}</h3>

    <h2>next</h2>
        <ul>
            {#each upcomingSongs as song}
                <li>{song.title}</li>
            {/each}
        </ul>
</div>

<style>
    .queue {
        position: fixed;
        bottom: 2em;
        right: 2em;
        width: 20%;
        height: 40%;
        background-color: #111;
        color: white;
        padding: 20px;
        overflow: auto;
    }
</style>