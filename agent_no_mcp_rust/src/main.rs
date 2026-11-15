fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open web server, port 5000, only endpoint "/"
    // Request is json {"prompt": (user prompt)}
    // User prompt sent to mainloop::run() and returned as json {"response": (agent response)} to client
    let port = 5000;
    let server = tiny_http::Server::http(format!("localhost:{}", port)).unwrap();

    loop {
        let mut request = server.recv().unwrap();
        if request.url() == "/" && request.method() == &tiny_http::Method::Post {
            let mut content = String::new();
            request.as_reader().read_to_string(&mut content).unwrap();
            let json: serde_json::Value = serde_json::from_str(&content).unwrap();
            let prompt = json["prompt"].as_str().unwrap_or("");
            let response = agent_no_mcp_rust::mainloop::run(prompt);
            // resolve response
            if response.is_err() {
                let error_response = serde_json::json!({ "error": response.err().unwrap().to_string() });
                let error_response_string = serde_json::to_string(&error_response).unwrap();
                let response = tiny_http::Response::from_string(error_response_string)
                    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                request.respond(response).unwrap();
                continue;
            }
            let response_json = serde_json::json!({ "response": response.unwrap() });
            let response_string = serde_json::to_string(&response_json).unwrap();
            let response = tiny_http::Response::from_string(response_string)
                .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
            request.respond(response).unwrap();
        } else {
            let response = tiny_http::Response::from_string("Not Found").with_status_code(404);
            request.respond(response).unwrap();
        }
    }
}