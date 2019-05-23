extern crate ws;
extern crate serde_json;
extern crate redis;
use redis::Commands;

use std::rc::Rc;
use std::convert::From;

mod message;
use message::Message;
use crate::message::Message::ClientRegister;

#[derive(Debug, PartialEq)]
enum Session {
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

#[derive(Debug, Clone)]
enum SessionEvent {
    RegisterClient { socket: Rc<ws::Sender> },
    RegisterAppStream { socket: Rc<ws::Sender> },
    Validate,
    Invalidate,
}


impl Session {
    fn next(self, event: SessionEvent) -> Session {
        match (self, event) {
            (Session::NotConnected, SessionEvent::RegisterClient { socket }) => {
                Session::ClientRegistered { socket }
            },
            (Session::NotConnected, SessionEvent::RegisterAppStream { socket}) => {
                Session::AppStreamRegistered { socket }
            },
            (Session::ClientRegistered { socket : client }, SessionEvent::RegisterAppStream { socket : app_stream }) => {
                Session::PendingValidation {
                    client: Rc::clone(&client),
                    app_stream: Rc::clone( &app_stream),
                }
            }
            (s, e) => {
                Session::Failure(format!("Wrong state, event combination: {:#?} {:#?}", s, e)
                    .to_string())
            }
        }
    }
}

struct Server {
    out: Rc<ws::Sender>,
    session: Session,
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
        println!("Server got message '{}' from {:?}. ", msg, self.out.token());

        let result : Result<Message, serde_json::Error> = serde_json::from_str(msg.as_text().unwrap());
        match result {
            Ok(value) => {
                match value {
                    Message::Ping {} => self.out.send(serde_json::to_string(&Message::Pong {}).unwrap()),
                    Message::ClientRegister {data} => {
                        println!("{:?}", data.key);
                        self.session = self.session.next(SessionEvent::RegisterClient { socket: Rc::clone(&self.out)});
                        Ok(())
                    },
                    Message::AppStreamRegister {data} => {
//                        self.state = SessionState::AppStreamRegistered {
//                            socket: Rc::clone(&self.out),
//                        };
                        Ok(())
                    },
                    _ => self.out.close(ws::CloseCode::Unsupported),
                }
            },
            Err(_error) => self.out.close(ws::CloseCode::Invalid),
        }
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        println!("WebSocket closing for ({:?}) {}", code, reason);
    }
}

fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let con = client.get_connection().unwrap();
    let _ : () = con.set("my_key", 42).unwrap();
    let r : i32 = con.get("my_key").unwrap();
    println!("{}", r);

    if let Err(error) = ws::listen("127.0.0.1:3012", |out| {
        Server {
            out: Rc::new(out),
            session: Session::NotConnected,
        }
    }) {
        println!("Failed to create WebSocket due to {:?}", error);
    }
}
