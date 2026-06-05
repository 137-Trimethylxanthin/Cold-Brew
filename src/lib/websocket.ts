import type { Song } from './types';

export class WebSocketHandler {
	private static instance: WebSocketHandler;
	private ws: WebSocket | null = null;

	private constructor() {}

	private connect() {
		if (this.ws && this.ws.readyState !== WebSocket.CLOSED) {
			return this.ws;
		}

		this.ws = new WebSocket('ws://localhost:6969');

		this.ws.onopen = () => {
			console.log('Connected to the websocket server');
		};

		this.ws.onmessage = (event) => {
			console.log('Message received:');
			console.log(event.data);
		};

		this.ws.onclose = () => {
			console.log('Disconnected from the websocket server');
		};

		return this.ws;
	}

	reconnect() {
		this.ws?.close();
		this.ws = null;
		return this.connect();
	}

	sendMessage(message: unknown) {
		const ws = this.connect();
		if (ws.readyState === WebSocket.OPEN) {
			ws.send(JSON.stringify(message));
		} else {
			console.error('Cannot send message, WebSocket is not open');
		}
	}

	public sendSong(song: Song) {
		this.sendMessage({
			command: '/add',
			song
		});
	}

	public getWebSocket() {
		return this.connect();
	}

	public static getInstance(): WebSocketHandler {
		if (!WebSocketHandler.instance) {
			WebSocketHandler.instance = new WebSocketHandler();
		}

		return WebSocketHandler.instance;
	}
}

export const wsh = {
	sendMessage(message: unknown) {
		WebSocketHandler.getInstance().sendMessage(message);
	},
	sendSong(song: Song) {
		WebSocketHandler.getInstance().sendSong(song);
	},
	getWebSocket() {
		return WebSocketHandler.getInstance().getWebSocket();
	},
	reconnect() {
		return WebSocketHandler.getInstance().reconnect();
	}
};
