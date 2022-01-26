use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::fs;
use crate::server::threadpool::ThreadPool;

mod threadpool;

pub fn main(address: &String) {
    println!("starting server...");
    let listener = TcpListener::bind(address).unwrap();
    let threadpool = ThreadPool::new(4);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => threadpool.execute(|| handle_connection(stream)),
            Err(e) => {
                println!("An error occurred when connecting to the client! Luckily, they'll probably try to connect again. {}", e);
            }
        }
    }
}

enum SendMethod {
    Binary,
    PlainText
}

enum Response {
    Binary(Vec<u8>),
    PlainText(String)
}

fn get_resource(url: String) -> Result<(SendMethod, String), String> {
    let path: Vec<&str> = url.split("/").into_iter().filter(|s| !s.is_empty()).collect();
    // println!("{:?}", path);
    if path.len() > 0 {
        let last_arg = path.last().unwrap();
        if last_arg.ends_with(".js") {
            Ok((SendMethod::PlainText, format!("scripts/{}", last_arg)))
        } else if vec![".html", ".css"].iter().any(|s| last_arg.ends_with(s)) {
            Ok((SendMethod::PlainText, format!("layout/{}", last_arg)))
        } else if vec![".jpg", ".ico"].iter().any(|s| last_arg.ends_with(s)) {
            Ok((SendMethod::Binary, format!("layout/{}", last_arg)))
        } else {
            Err(format!("Don't know how to look for resource at {}", url))
        }
    } else {
        Ok((SendMethod::PlainText, String::from("layout/index.html")))
    }
}
/**
HTTP Format:
```
data: [GET|SET|POST] URL HTTP/[HTTP Version]\r\n
Header-Key: Header-Value\r\n
...
Content-Length: [length in bytes]\r\n
\r\n [notice double CRLF]
[content with content length in bytes]
```
*/
fn handle_connection(mut stream: TcpStream) {
    // println!("something connected here");
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    println!("data: {}", String::from_utf8_lossy(&buffer[..]));
    let data_as_string: String = String::from_utf8_lossy(&buffer[..]).into();
    let response = match data_as_string.split("\r\n").next() {
        Some(line) => {
            let args = line.split(" ").collect::<Vec<_>>();
            if args.len() <3 {
                create_bad_request_error("Badly formatted HTTP request.".to_string())
            } else {
                let message_type = args[0];
                let url = args[1];
                let http_version = args[2];
                match message_type {
                    "GET" => handle_get(url),
                    "PUT" => {
                        create_bad_request_error("no data to put idiot".to_string())
                    },
                    _ => {
                        create_bad_request_error("what are you even trying to do".to_string())
                    }
                }
            }
        },
        None => create_bad_request_error("Malformatted request.".to_string())
    };
    match response {
        Response::PlainText(string) => {
            stream.write(string.as_bytes()).unwrap();
        },
        Response::Binary(data) => {
            stream.write(data.as_slice()).unwrap();
        }
    };
    stream.flush().unwrap();
}

fn handle_get(url: &str) -> Response {
    match get_resource(url.to_string()) {
        Ok((send_method, resource_path)) => match send_method {
            SendMethod::PlainText =>
                match fs::read_to_string(resource_path) {
                    Ok(resource_file) => Response::PlainText(format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                        resource_file.len(),
                        resource_file
                    )),
                    Err(err) => create_bad_request_error(
                        err.to_string()
                    )
                },
            SendMethod::Binary =>
                match fs::read(resource_path) {
                    Ok(binary_data) => {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                            binary_data.len());
                        let mut data = Vec::with_capacity(header.len() + binary_data.len());
                        for c in header.as_bytes() {
                            data.push(*c);
                        }
                        for b in binary_data {
                            data.push(b);
                        }
                        Response::Binary(data)
                    },
                    Err(err) => create_bad_request_error(err.to_string())
                }
        },
        Err(error_message) => create_bad_request_error(
            format!("Cannot handle GET Request. {}", error_message))
    }
}

fn create_bad_request_error(description: String) -> Response {
    Response::PlainText(format!("HTTP/1.1 400 {}\r\n\r\n", description))
}