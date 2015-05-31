extern crate mio;

use std::net::SocketAddr;
use std::net::Shutdown;
use std::rc::Rc;
use std::cell::RefCell;

use mio::buf::RingBuf;
use mio::util::Slab;
use mio::tcp::{TcpListener, TcpStream};
use mio::{EventLoop, Buf,  Handler, Token, ReadHint, NonBlock, IntoNonBlock, 
    Interest, PollOpt, TryWrite, TryRead};

struct Transport {
    client_sock  : NonBlock<TcpStream>,
    client_token : Token,
    server_sock  : NonBlock<TcpStream>,
    server_token : Token,
    rx_buf       : RingBuf,
    tx_buf       : RingBuf,
} 

impl Transport {
    fn new(client_sock: NonBlock<TcpStream>, server_sock: NonBlock<TcpStream>) -> Transport {
        Transport{
            client_sock : client_sock,
            client_token : Token(0),
            server_sock : server_sock,
            server_token: Token(0),
            rx_buf : RingBuf::new(4096),
            tx_buf :RingBuf::new(4096),
        }
    }

    fn client_readable(&mut self, el: &mut EventLoop<RoxyServer>, hint: ReadHint) {
        println!("client {:?} is readable", self.client_token);
        self.client_sock.read(&mut self.rx_buf).unwrap();

        if Buf::has_remaining(&self.rx_buf) {
            el.reregister(&self.server_sock, self.server_token, Interest::all(), PollOpt::level()).unwrap();
        }

        if hint.is_hup() {
            self.client_sock.shutdown(Shutdown::Both);
            self.server_sock.shutdown(Shutdown::Both);
            el.deregister(&self.client_sock).unwrap();
            el.deregister(&self.server_sock).unwrap();
        }
    }

    fn client_writable(&mut self, el: &mut EventLoop<RoxyServer>) {
        println!("client {:?} is writable", self.client_token);
        self.client_sock.write(&mut self.tx_buf).unwrap();

        if !Buf::has_remaining(&self.tx_buf) {
            el.reregister(&self.client_sock, self.client_token, readable(), PollOpt::level()).unwrap();
        }
    }

    fn server_readable(&mut self, el: &mut EventLoop<RoxyServer>, hint: ReadHint) {
        println!("server {:?} is readable", self.server_token);
        self.server_sock.read(&mut self.tx_buf).unwrap();

        if Buf::has_remaining(&self.tx_buf) {
            el.reregister(&self.client_sock, self.client_token, Interest::all(), PollOpt::level()).unwrap();
        }

        if hint.is_hup() {
            self.client_sock.shutdown(Shutdown::Both).unwrap();
            self.server_sock.shutdown(Shutdown::Both).unwrap();
            el.deregister(&self.client_sock).unwrap();
            el.deregister(&self.server_sock).unwrap();
        }
    }

    fn server_writable(&mut self, el: &mut EventLoop<RoxyServer>) {
        println!("server {:?} is writable", self.server_token);
        self.server_sock.read(&mut self.tx_buf).unwrap();
        self.server_sock.write(&mut self.rx_buf).unwrap();

        if !Buf::has_remaining(&self.rx_buf) {
            el.reregister(&self.server_sock, self.server_token, readable(), PollOpt::level()).unwrap();
        }
    }

    fn readable(&mut self, el: &mut EventLoop<RoxyServer>, token: Token, hint: ReadHint) {
        match token {
            c if token == self.client_token => self.client_readable(el, hint),
            d if token == self.server_token => self.server_readable(el, hint),
            _ => panic!("what's the hell?"),
        }
    }

    fn writable(&mut self, el: &mut EventLoop<RoxyServer>, token: Token) {
        match token {
            c if token == self.client_token => self.client_writable(el),
            d if token == self.server_token => self.server_writable(el),
            _ => panic!("what's the hell?"),
        }
    }
}


pub struct RoxyServer {
    src        : SocketAddr,
    dst        : SocketAddr,
    srv        : Option<NonBlock<TcpListener>>,
    trans      : Option<Transport>,
    transports : Slab<Rc<RefCell<Transport>>>,
}

impl RoxyServer {
    pub fn new(src: SocketAddr, dst: SocketAddr) ->  RoxyServer {
        RoxyServer {
            src: src, 
            dst: dst, 
            srv: None,
            trans: None,
            transports: Slab::new_starting_at(Token(1024), 4096),
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
        match token {
            PROXY => {
                let client_sock = self.srv.as_mut().unwrap().accept().unwrap().unwrap();
                let server_sock = TcpStream::connect(self.dst).unwrap().into_non_block().unwrap();

                let ref_trans = Rc::new(RefCell::new(Transport::new(client_sock, server_sock)));
                let client_token = self.transports.insert(ref_trans.clone()).ok().expect("slab is full");
                let server_token = self.transports.insert(ref_trans.clone()).ok().expect("slab is full");

                let mut trans = self.transports[client_token].borrow_mut();
                trans.client_token = client_token;
                trans.server_token = server_token;

                el.register_opt(&trans.client_sock, client_token, readable(), PollOpt::level()).unwrap();
                el.register_opt(&trans.server_sock, server_token, readable(), PollOpt::level()).unwrap();
            },

            _ => {
                let mut trans = self.transports[token].borrow_mut();
                trans.readable(el, token, hint);
            }

        }
    }

    fn writable(&mut self, el: &mut EventLoop<Self>, token: Token) {
        let mut trans = self.transports[token].borrow_mut();
        trans.writable(el, token);
    }
}
