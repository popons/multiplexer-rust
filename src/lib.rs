use std::sync::mpsc::{Sender, channel, Receiver};
use std::thread::{JoinHandle, spawn};

pub trait Source <TOUT: Send> : 'static + Send {
    fn fanin(&mut self, sink: Sender<TOUT>);
}

pub fn start_multiplex<TOUT: 'static + Send> (sources: Vec<Box<Source<TOUT>>>) -> Receiver<TOUT>
    where Source<TOUT>: Send {
    let (sink, fanout) = channel();
    let _ = sources.into_iter().map(|mut src| {
        let sink = sink.clone();
        spawn(move || {
            src.fanin(sink)
        })
    }).collect::<Vec<_>>();
    return fanout;
}

#[test]
fn it_works() {
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::thread;
    use std::intrinsics::{type_name};
    use std::error;
    use std::net::{TcpStream, TcpListener};
    use std::io::{Read, Write};

    #[derive(PartialEq,Eq,Debug)]
    struct A;
    #[derive(PartialEq,Eq,Debug)]
    struct B;
    #[derive(PartialEq,Eq,Debug)]
    struct C(String);
    #[derive(PartialEq,Eq,Debug)]
    enum ABC {
        A(A),
        B(B),
        C(C),
    }

    impl Source<ABC> for Receiver<A> {
        fn fanin(&mut self, sink: Sender<ABC>) {
            loop {
                if let Ok(x) = self.recv() {
                    sink.send(ABC::A(x));
                } else {
                    return;
                }
            }
        }
    }

    impl Source<ABC> for Receiver<B> {
        fn fanin(&mut self, sink: Sender<ABC>) {
            loop {
                if let Ok(x) = self.recv() {
                    sink.send(ABC::B(x));
                } else {
                    return;
                }
            }
        }
    }

    impl Source<ABC> for TcpStream {
        fn fanin(&mut self, sink: Sender<ABC>) {
            loop {
                let mut b = [0; 128];
                match self.read(&mut b) {
                    Ok(n) => {
                        sink.send(ABC::C(C(String::from_utf8_lossy(&b[0..n]).into_owned())));
                    }
                    Err(e) => {
                        println!("{:?}", e);
                    }
                }
            }
        }
    }

    let (input_a, _src_a) = channel::<A>();
    let (input_b, _src_b) = channel::<B>();

    // start echo server
    thread::spawn(|| {
        let lis = TcpListener::bind("0.0.0.0:54321").unwrap();
        for x in lis.incoming() {
            if let Ok(mut stream) = x {
                let mut buf = [0; 128];
                if let Ok(n) = stream.read(&mut buf) {
                    stream.write(&buf[0..n]);
                }
            }
        }
    });

    // start echo client
    let mut tcp_client = TcpStream::connect("127.0.0.1:54321").unwrap();

    let mut src_a: Box<Source<ABC>> = Box::new(_src_a);
    let mut src_b: Box<Source<ABC>> = Box::new(_src_b);
    let mut src_c: Box<Source<ABC>> = Box::new(tcp_client.try_clone().unwrap());

    let fanout = start_multiplex(vec![src_a, src_b, src_c]);

    input_a.send(A);
    thread::sleep_ms(100);
    input_b.send(B);
    thread::sleep_ms(100);
    tcp_client.write("1234".as_bytes());
    thread::sleep_ms(100);

    let a = fanout.recv().unwrap();
    let b = fanout.recv().unwrap();
    let c = fanout.recv().unwrap();

    assert_eq!(a, ABC::A(A));
    assert_eq!(b, ABC::B(B));
    assert_eq!(c, ABC::C(C("1234".to_string())));

}
