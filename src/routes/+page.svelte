<script>
    import { invoke } from '@tauri-apps/api/core';

    // Invoke the command
    let res;
    async function getMyData(){
        res = await invoke("display_song_list")
        console.log(res)
    }
    console.log(        "hiiii")


</script>

<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://kit.svelte.dev">kit.svelte.dev</a> to read the documentation</p>
