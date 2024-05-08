<script lang="ts">
	import { goto } from '$app/navigation';
    import { invoke } from "@tauri-apps/api/core";
    import type { Song } from "$lib/types";
    import { onMount } from "svelte";
    import { wsh } from "$lib/websocket";


    let currentSong: Song = {title: "loading", artist: "loading", album: "loading", duration: 0, id: "loading"};
    let oldSongs: Song[] = [];
    let upcomingSongs: Song[] = [];

    let ws: WebSocket;

       // Connect to my WebSocket
    onMount(() => {
    

        ws = wsh.getWebSocket();


            // Listen for messages
        ws.addEventListener('message', event => {
            let json = JSON.parse(event.data);
            console.log("got message");
            console.log(json);

            if (json.current_song != null) {
                currentSong = {
                    title: json.current_song.title,
                    artist: json.current_song.artist,
                    album: json.current_song.album,
                    duration: json.current_song.duration,
                    id: json.current_song.id
                };
            }

            if (json.old != null) {
                oldSongs = json.old.map((song: any) => pasreJason(song));
            }
            if (json.upcoming != null) {
                upcomingSongs = json.upcoming.map((song: any) => pasreJason(song));
            }
        });
    });



    function pasreJason(json: any): Song {
        return {
            title: json.title,
            artist: json.artist,
            album: json.album,
            duration: json.duration,
            id: json.id
        };
    }


</script>

<!-- Side Nav -->

<div class="sidenav">
    <button on:click={() => goto("/")}>Home</button>
    <button on:click={() => goto("player")}>Player</button>
    <button>reload Queue</button>
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


<div class="miniPlayer">
    <div>
        <img src={currentSong.id} alt="">
        <h1>{currentSong.title}</h1>
        <h2>{currentSong.artist}</h2>
        <h3>{currentSong.album}</h3>
    </div>
    <span>{currentSong.duration}</span>
    <div class="durationBar" ></div>
    <div>
        <button on:click={() => ws.send(JSON.stringify({"command": "/next", "song": "none"}))}>Next</button>
        <button on:click={() => ws.send(JSON.stringify({"command": "/pause", "song": "none"}))}>Pause</button>
        <button on:click={() => ws.send(JSON.stringify({"command": "/previous", "song": "none"}))}>Prev</button>
    </div>
</div>

<style>
    .queue {
        position: fixed;
        bottom: 12em;
        right: 2em;
        width: 20%;
        height: 40%;
        background-color: #111;
        color: white;
        padding: 20px;
        overflow: auto;
    }

    .miniPlayer {
        position: fixed;
        bottom: 2em;
        left: 10%;
        width: 80%;
        height: 10%;
        background-color: #111;
        color: white;
        padding: 20px;
        overflow: auto;
        display: flex;
        justify-content: space-between;
        flex-direction: row;
    }

    .miniPlayer div {
        display: flex;
        justify-content: space-between;
        flex-direction: row;
    }

    .miniPlayer img {
        width: 100%;
        height: 100%;
    }

    .durationBar {
        width: 100%;
        height: 10px;
        background-color: #333;
    }
</style>