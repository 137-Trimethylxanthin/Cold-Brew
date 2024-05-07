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


    // Connect to my WebSocket
    const ws = new WebSocket('ws://localhost:6969');


    // Listen for messages
    ws.addEventListener('message', event => {
        //check what type of message it is
        console.log("data: ");
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
            call: "Next",
            data: song
        };
        console.log("Sending message");
        ws.send(JSON.stringify(message));
        console.log("Message sent");
    });



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

