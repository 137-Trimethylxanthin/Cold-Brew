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
        const data = JSON.parse(event.data);
        console.log("data: "+data);
    });


    ws.addEventListener('open', () => {
        ws.send(JSON.stringify(
            {
                type: "message",
                payload: "Hello from the client!"
            }
        ));
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

