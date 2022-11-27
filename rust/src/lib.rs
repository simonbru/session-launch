use std::io::{BufRead, Write};

pub fn run_stream_pipe(mut source: Box<dyn BufRead>, mut dest: Box<dyn Write>) {
    loop {
        let buffer = source.fill_buf().unwrap();
        let length = buffer.len();
        if length == 0 {
            break;
        };
        dest.write(buffer).unwrap();
        source.consume(length);
    }
}
