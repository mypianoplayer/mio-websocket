/// High-level WebSocket library interface
extern crate slab;

use std::net::SocketAddr;
use std::thread;
use std::sync::mpsc;
use std::time::Duration;

use mio::{Token, EventLoop, EventSet, PollOpt, Sender, NotifyError};
use mio::tcp::{TcpListener};
use websocket_essentials::{StatusCode};

use server::{WebSocketServer, SERVER_TOKEN};

#[derive(Clone)]
pub enum WebSocketEvent {
    Connect,
    Close(StatusCode),
    Ping(Box<[u8]>),
    Pong(Box<[u8]>),
    TextMessage(String),
    BinaryMessage(Vec<u8>)
}

pub enum WebSocketInternalMessage {
    GetPeers(mpsc::Sender<Vec<Token>>),
    SendMessage((Token,WebSocketEvent)),
    Reregister(Token)
}

pub struct WebSocket {
    events: mpsc::Receiver<(Token,WebSocketEvent)>,
    event_loop_tx: Sender<WebSocketInternalMessage>
}

impl WebSocket {
    pub fn new(address: SocketAddr) -> WebSocket {
        let (tx, rx) = mpsc::channel();

        let mut event_loop = EventLoop::new().unwrap();
        let event_loop_tx = event_loop.channel();

        thread::spawn(move || {
            let server_socket = TcpListener::bind(&address).unwrap();
            let mut server = WebSocketServer::new(server_socket, tx);

            event_loop.register(&server.socket,
                                SERVER_TOKEN,
                                EventSet::readable(),
                                PollOpt::edge()).unwrap();

            event_loop.run(&mut server).unwrap();
        });

        WebSocket {
            event_loop_tx: event_loop_tx,
            events: rx
        }
    }

    pub fn next(&mut self) -> (Token,WebSocketEvent) {
        self.events.recv().unwrap()
    }

    pub fn try_next(&mut self) -> Result<(Token,WebSocketEvent),mpsc::TryRecvError> {
        self.events.try_recv()
    }

    pub fn get_connected(&mut self) -> Result<Vec<Token>, mpsc::RecvError> {
        let (tx, rx) = mpsc::channel();
        self.send_internal(WebSocketInternalMessage::GetPeers(tx));
        rx.recv()
    }

    pub fn send(&mut self, msg: (Token,WebSocketEvent)) {
        self.send_internal(WebSocketInternalMessage::SendMessage(msg));
    }
    pub fn send_peer(&mut self, tok:usize, msg:String) {
        println!("sending {} to {}", msg, tok);
        let evt = WebSocketEvent::TextMessage(msg.clone());
        self.send((Token(tok),evt));
    }
    pub fn send_all(&mut self, msg: String) {
        println!("sending {} to all", msg);
        for peer in self.get_connected().unwrap() {
            let evt = WebSocketEvent::TextMessage(msg.clone());
            self.send((peer, evt));
        }
    }

    fn send_internal(&mut self, msg: WebSocketInternalMessage) -> Result<(), NotifyError<WebSocketInternalMessage>> {
        let mut val = msg;
        loop {
            match self.event_loop_tx.send(val) {
                Err(NotifyError::Full(ret)) => {
                    // The notify queue is full, retry after some time.
                    val = ret;
                    thread::sleep(Duration::from_millis(10));
                },
                result @ _ => return result,
            }
        }
    }
}
