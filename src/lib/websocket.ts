import { writable } from 'svelte/store';
import type { Song } from './types';



export class WebSocketHandler {
    private static instance: WebSocketHandler;
    private ws: WebSocket;

    private constructor() {
        this.ws = new WebSocket('ws://localhost:6969');

        this.ws.onopen = () => {
            console.log('Connected to the websocket server');
        }

        this.ws.onmessage = (event) => {
            console.log('Message received:');
            console.log(event.data);
        }

        this.ws.onclose = () => {
            console.log('Disconnected from the websocket server');
        }
    }

    reconnect() {
        this.ws = new WebSocket('ws://localhost:6969');

        this.ws.onopen = () => {
            console.log('Connected to the websocket server');
        }

        this.ws.onmessage = (event) => {
            console.log('Message received:');
            console.log(event.data);
        }

        this.ws.onclose = () => {
            console.log('Disconnected from the websocket server');
        }
    }

    sendMessage(message: any) {
        if (this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            console.error('Cannot send message, WebSocket is not open');
        }
    }

    public sendSong(song: Song) {
        let message = {
                "command": "/add",
                "song": song,
        };
        this.sendMessage(message);
    }

    public getWebSocket() {
        return this.ws;
    }

    public static getInstance(): WebSocketHandler {
        if (!WebSocketHandler.instance) {
            WebSocketHandler.instance = new WebSocketHandler();
        }

        return WebSocketHandler.instance;
    }

}

// Export the singleton instance
export const wsh = WebSocketHandler.getInstance();