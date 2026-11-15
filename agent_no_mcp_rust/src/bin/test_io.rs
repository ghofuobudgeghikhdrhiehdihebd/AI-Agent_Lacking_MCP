use agent_no_mcp_rust::serial_utils::ArduinoSerial;
use std::time::Duration;

fn main() {
    let mut uart = ArduinoSerial::new().expect("Failed to initialize Arduino UART");
    uart.send_text("Test message")
        .expect("Failed to send text over UART");

    println!("Waiting up to 5 seconds for an unframed UTF-8 serial response...");
    let data = uart
        .receive_until_idle(Duration::from_secs(5), Duration::from_millis(100))
        .expect("Failed to receive a serial response");

    println!("Received {} raw bytes: {data:02X?}", data.len());
    let response = String::from_utf8(data).expect("Received response is not valid UTF-8");
    println!("Received response: {response}");
}
