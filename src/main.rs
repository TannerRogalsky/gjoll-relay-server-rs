extern crate ws;
extern crate serde_json;
extern crate redis;
use redis::{Commands, Connection};

use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

mod message;
use message::{Message, RelayKey};

#[derive(Debug, PartialEq, Clone)]
pub enum Session {
    NotConnected,
    ClientRegistered { socket: Rc<ws::Sender> },
    AppStreamRegistered { socket: Rc<ws::Sender> },
    PendingValidation {
        client: Rc<ws::Sender>,
        app_stream: Rc<ws::Sender>,
    },
    Established {
        client: Rc<ws::Sender>,
        app_stream: Rc<ws::Sender>,
    },
    Failure(String),
}

impl Session {
    fn next(&self, event: &SessionEvent) -> Session {
        match (self, event) {
            (Session::NotConnected, SessionEvent::RegisterClient { socket }) => {
                println!("register_client");
                Session::ClientRegistered { socket: Rc::clone(socket) }
            },
            (Session::NotConnected, SessionEvent::RegisterAppStream { socket}) => {
                println!("app_stream_register");
                Session::AppStreamRegistered { socket: Rc::clone(socket) }
            },
            (Session::ClientRegistered { socket: client }, SessionEvent::RegisterAppStream { socket: app_stream }) => {
                println!("pending_validation");
                Session::Established {
                    client: Rc::clone(client),
                    app_stream: Rc::clone(app_stream),
                }
            },
            (Session::AppStreamRegistered { socket: app_stream }, SessionEvent::RegisterClient { socket: client }) => {
                println!("pending_validation");
                Session::Established {
                    client: Rc::clone(client),
                    app_stream: Rc::clone( app_stream),
                }
            }
            (s, e) => {
                Session::Failure(format!("Wrong state, event combination: {:#?} {:#?}", s, e))
            }
        }
    }
}

#[derive(Debug, Clone)]
enum SessionEvent {
    RegisterClient { socket: Rc<ws::Sender> },
    RegisterAppStream { socket: Rc<ws::Sender> },
//    Validate,
//    Invalidate,
}

pub struct Server {
    out: Rc<ws::Sender>,
    pending_sessions: Rc<RefCell<HashMap<RelayKey, ws::util::Token>>>,
    sessions: Rc<RefCell<HashMap<ws::util::Token, Rc<RefCell<Session>>>>>,
    redis: Rc<Connection>,
}

impl Server {
    fn coordinate_sessions(&mut self, relay_key: &str) {
        let mut sessions = self.sessions.borrow_mut();
        let mut pending_sessions = self.pending_sessions.borrow_mut();
        if let Some(token) = pending_sessions.remove(relay_key) {
            let s = Rc::clone(&sessions[&token]);
            sessions.insert(self.out.token(), s);
        } else {
            pending_sessions.insert(relay_key.to_string(), self.out.token());
        }
    }

    fn session_event(&mut self, token: &ws::util::Token, event: SessionEvent) {
        let sessions = self.sessions.borrow_mut();
        let mut session = sessions[token].borrow_mut();
        *session = session.next(&event);
    }
}

impl ws::Handler for Server {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if let Some(ip_addr) = shake.remote_addr()? {
            println!("Connection opened from {}.", ip_addr)
        } else {
            println!("Unable to obtain client's IP address.")
        }
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        { // debug
            let mut sessions = self.sessions.borrow_mut();
            let session = sessions.entry(self.out.token())
                .or_insert(Rc::new(RefCell::new(Session::NotConnected)));
            println!("{:?}, {:?}", Rc::strong_count(session), session);
        }


        let result : Result<Message, serde_json::Error> = serde_json::from_str(msg.as_text().unwrap());
        match result {
            Ok(value) => {
                // This should maybe be the thing that assigns new session state
                match value {
                    Message::Ping {} => self.out.send(serde_json::to_string(&Message::Pong {}).unwrap()),
                    Message::ClientRegister {data} => {
                        self.coordinate_sessions(&data.key);
                        self.session_event(
                            &self.out.token(),
                            SessionEvent::RegisterClient { socket: Rc::clone(&self.out) }
                        );
                        Ok(())
                    },
                    Message::AppStreamRegister {data} => {
                        self.coordinate_sessions(&data.key);
                        self.session_event(
                            &self.out.token(),
                            SessionEvent::RegisterAppStream { socket: Rc::clone(&self.out) }
                        );
                        Ok(())
                    },
                    _ => {
                        let sessions = self.sessions.borrow_mut();
                        let session = sessions[&self.out.token()].borrow_mut();
                        match &*session {
                            Session::NotConnected |
                            Session::ClientRegistered { .. } |
                            Session::AppStreamRegistered { .. } |
                            Session::PendingValidation { .. } |
                            Session::Failure(_) => self.out.close(ws::CloseCode::Unsupported),
                            Session::Established { client, app_stream } => {
                                if self.out.token() == client.token() {
                                    app_stream.send(msg)
                                } else {
                                    client.send(msg)
                                }
                            },
                        }
                    },
                }
            },
            Err(_error) => self.out.close(ws::CloseCode::Invalid),
        }
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        println!("WebSocket closing for ({:?}) {}", code, reason);
        if let Some(session) = self.sessions.borrow_mut().remove(&self.out.token()) {
            println!("removed session: {:?}", session);
        }
    }
}

fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let con = Rc::new(client.get_connection().unwrap());
    let _ : () = con.set("my_key", 42).unwrap();
    let r : i32 = con.get("my_key").unwrap();
    println!("{}", r);

    let pending_sessions: Rc<RefCell<HashMap<RelayKey, ws::util::Token>>> = Rc::new(RefCell::new(HashMap::new()));
    let sessions: Rc<RefCell<HashMap<ws::util::Token, Rc<RefCell<Session>>>>> = Rc::new(RefCell::new(HashMap::new()));

    if let Err(error) = ws::listen("127.0.0.1:3012", |out| {
        Server {
            out: Rc::new(out),
            pending_sessions: Rc::clone(&pending_sessions),
            sessions: Rc::clone(&sessions),
            redis: Rc::clone(&con),
        }
    }) {
        println!("Failed to create WebSocket due to {:?}", error);
    }
}
