extern crate mio;

use std::net::SocketAddr;
use std::net::Shutdown;

use mio::buf::RingBuf;
use mio::tcp::{TcpListener, TcpStream};
use mio::{EventLoop, Buf,  Handler, Token, ReadHint, NonBlock, IntoNonBlock, 
    Interest, PollOpt, TryWrite, TryRead};

pub struct Transport {
    sock_client : NonBlock<TcpStream>,
    rx_buf      : RingBuf,
    sock_server : NonBlock<TcpStream>,
    tx_buf      : RingBuf,
}

pub struct RoxyServer {
    src     : SocketAddr,
    dst     : SocketAddr,
    srv     : Option<NonBlock<TcpListener>>,
    trans   : Option<Transport>,
}

impl RoxyServer {
    pub fn new(src: SocketAddr, dst: SocketAddr) ->  RoxyServer {
        RoxyServer {
            src: src, 
            dst: dst, 
            srv: None,
            trans: None,
        }
    }

    pub fn run(&mut self) {
        let mut el = EventLoop::new().unwrap();
        let srv = TcpListener::bind(&self.src).unwrap();

        el.register(&srv, Token(0)).unwrap();
        self.srv = Some(srv.into_non_block().unwrap());

        el.run(self).unwrap();

    }
}

const PROXY: Token = Token(0);
const CLIENT: Token = Token(1);
const SERVER: Token = Token(2);

#[inline]
fn readable () -> Interest { 
    Interest::readable() | Interest::hinted() | Interest::hup() 
}

impl Handler for RoxyServer {
    type Timeout = u64;
    type Message = u32;

    fn readable(&mut self, el: &mut EventLoop<Self>, token: Token, hint: ReadHint) {
        println!("{:?} is readable", token);
        match token {
            PROXY => {
                let sock_client = self.srv.as_mut().unwrap().accept().unwrap().unwrap();
                let sock_server = TcpStream::connect(self.dst).unwrap().into_non_block().unwrap();

                el.register_opt(&sock_client, CLIENT, readable(), PollOpt::level()).unwrap();
                el.register_opt(&sock_server, SERVER, readable(), PollOpt::level()).unwrap();

                self.trans = Some(Transport {
                    sock_client : sock_client,
                    rx_buf : RingBuf::new(4096),
                    sock_server : sock_server,
                    tx_buf : RingBuf::new(4096),

                });
            },

            CLIENT => {
                let trans = self.trans.as_mut().unwrap();
                let sock_client = &mut trans.sock_client;
                let sock_server = &mut trans.sock_server;
                let rx_buf = &mut trans.rx_buf;

                sock_client.read(rx_buf).unwrap();

                if Buf::has_remaining(rx_buf) {
                    el.reregister(sock_server, SERVER, Interest::all(), PollOpt::level()).unwrap();
                }

                if hint.is_hup() {
                    sock_client.shutdown(Shutdown::Both).ok().expect("fail to close client socket");
                    sock_server.shutdown(Shutdown::Both).unwrap();
                    el.deregister(sock_client).unwrap();
                    el.deregister(sock_server).unwrap();
                    println!("client close");
                }
            },

            SERVER => {
                let trans = self.trans.as_mut().unwrap();
                let sock_client = &mut trans.sock_client;
                let sock_server = &mut trans.sock_server;
                let tx_buf = &mut trans.tx_buf;
                sock_server.read(tx_buf).unwrap();

                if Buf::has_remaining(tx_buf) {
                    el.reregister(sock_client, CLIENT, Interest::all(), PollOpt::level()).unwrap();
                }

                if hint.is_hup() {
                    sock_client.shutdown(Shutdown::Both).unwrap();
                    sock_server.shutdown(Shutdown::Both).unwrap();
                    el.deregister(sock_client).unwrap();
                    el.deregister(sock_server).unwrap();
                    println!("server close");
                }
            },
            _ => panic!("oops"),
        }
    }

    fn writable(&mut self, el: &mut EventLoop<Self>, token: Token) {
        println!("{:?} is writable", token);
        match token {
            CLIENT => {
                let trans = self.trans.as_mut().unwrap();
                let sock_client = &mut trans.sock_client;
                let tx_buf = &mut trans.tx_buf;
                sock_client.write(tx_buf).unwrap();

                if !Buf::has_remaining(tx_buf) {
                    el.reregister(sock_client, CLIENT, readable(), PollOpt::level()).unwrap();
                }
            },
            SERVER => {
                let trans = self.trans.as_mut().unwrap();
                let sock_server = &mut trans.sock_server;
                let rx_buf = &mut trans.rx_buf;
                sock_server.write(rx_buf).unwrap();

                if !Buf::has_remaining(rx_buf) {
                    el.reregister(sock_server, SERVER, readable(), PollOpt::level()).unwrap();
                }
            },
            _ => panic!("oops"),
        };
    }
}
