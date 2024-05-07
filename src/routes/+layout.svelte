<script lang="ts">
	import { goto } from '$app/navigation';
    import { invoke } from "@tauri-apps/api/core";
    import type { Song } from "$lib/types";


    let currentSong: Song = {title: "loading", artist: "loading", album: "loading", duration: 0, id: "loading"};
    let oldSongs: Song[] = [];
    let upcomingSongs: Song[] = [];

       // Connect to my WebSocket
    const ws = new WebSocket('ws://localhost:6969');


    // Listen for messages
    ws.addEventListener('message', event => {
        //check what type of message it is
        console.log("data: ");
        let json = JSON.parse(event.data);
        console.log(json);

        if (json.current_song != null) {
            currentSong = {
                title: json.current_song.title,
                artist: json.current_song.artist,
                album: json.current_song.album,
                duration: json.current_song.duration,
                id: json.current_song.id
            };
            console.log("current song: ");
            console.log(currentSong);
        }

        if (json.old_songs != null) {
            oldSongs = json.old_songs;
        }
        if (json.upcoming_songs != null) {
            upcomingSongs = json.upcoming_songs;
        }
    });


    ws.addEventListener('open', () => {
        let song = {
            id: "1",
            title: "Song Title",
            artist: "Artist Name",
            album: "Album Name",
            duration: 300
            };
        let message = {
            "command": "/add",
            "song": song,
        };
        console.log("Sending message");
        console.log(JSON.stringify(message));
        ws.send(JSON.stringify(message));
        console.log("Message sent");

        ws.send(JSON.stringify({"command": "/next", "song": "none"}));

        ws.send(JSON.stringify({"command": "/get_queue", "song": "none"}));
    });

    console.log('Hello from the Layout!');



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