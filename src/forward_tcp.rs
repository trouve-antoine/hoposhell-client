use std::{net::TcpListener, io::{Read, Write}, time::Duration};

use crate::{args::Args, commands::{tcp, send_command_handler::{connect_to_hoposhell, send_request_and_get_response}, request_or_response::StatusCode}, connect::compute_hostname, make_random_id, constants::BUF_SIZE};

pub fn main_forward_tcp(args: Args) {
    /* Create a server that forwards all access to a port
     * to a remote shell */

    /* hopo forward <local port> <shell_id> <remote_port> */

    let local_port = args.extra_args[0].parse::<u16>().unwrap();
    let host = args.extra_args[1].clone();
    let remote_port = args.extra_args[2].parse::<u16>().unwrap();

    let listener = TcpListener::bind(format!("localhost:{}", local_port)).unwrap();
    eprintln!("Wait for connection at port {}", local_port);

    for stream in listener.incoming() {
        eprintln!("Got incomming connection");
        let mut stream = stream.unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

        let mut buf = [0; BUF_SIZE];
        // let read_size = stream.read_to_end(&mut buf);
        let read_size = stream.read(&mut buf);

        if read_size.is_err() {
            eprintln!("- failed to read stream.");
            continue;
        }

        eprintln!("- read {} bytes", read_size.unwrap());

        /* Generate tcp request */
        let shell_id = args.shell_name.clone().unwrap();
        let make_id = || {  
            let random_str = make_random_id(8);
            return format!("{}:{}", &shell_id, random_str)
        };
        let req = tcp::make_tcp_request(make_id, &shell_id, host.clone(), remote_port, buf.to_vec());
        
        /* Connect to shell and send TCP command */
        let (ssl_connector, tcp_stream) = connect_to_hoposhell(&args);

        /* Send request to hoposhell server */
        let res = if let Some(ref ssl_connector) = ssl_connector {
            let hostname = compute_hostname(&args.server_url);
            let ssl_stream = ssl_connector.connect(hostname, tcp_stream).unwrap();
            send_request_and_get_response(&args, ssl_stream, &req, args.verbose)
            // handle_command_connection(&args, ssl_stream, &req, &process_res, args.verbose)
        } else {
            send_request_and_get_response(&args, tcp_stream, &req, args.verbose)
        };

        /* Process response */
        if res.is_err() {
            eprintln!("Failed to send/recieve to/from hoposhell server: {}", res.err().unwrap());
            continue;
        }
        let res = res.unwrap();

        eprintln!("#########################");
        eprintln!("{}", String::from_utf8_lossy(&res.payload));
        eprintln!("#########################");

        if res.status_code != StatusCode::Ok {
            eprintln!("Got error status code from hoposhell server: {:?}", res.status_code);
            continue;
        }

        let write_res = stream.write_all(&res.payload);
        if write_res.is_err() {
            eprintln!("Failed to write back to stream: {}", write_res.err().unwrap());
            continue;
        }
        eprintln!("Wrote {} bytes back to stream", res.payload.len());
    }
}